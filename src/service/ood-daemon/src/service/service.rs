use super::service_info::*;
use crate::config::*;
use crate::repo::REPO_MANAGER;
use cyfs_base::{BuckyError, BuckyResult};
use ood_control::OOD_CONTROLLER;
use cyfs_util::{process::ProcessStatusCode};

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
struct ServiceInnerState {
    state: ServiceState,

    // 拉起服务进程后的进程对象，如果服务进程先于我们启动，则此处为空
    process: Option<Child>,
}

impl ServiceInnerState {
    fn new() -> Self {
        Self {
            state: ServiceState::STOP,
            process: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Service {
    name: String,
    fid: String,
    full_fid: String,

    info: Arc<Mutex<ServicePackageInfo>>,

    state: Arc<Mutex<ServiceInnerState>>,
}

impl Service {
    pub fn new(service_config: &ServiceConfig) -> Self {
        let info = ServicePackageInfo::new(service_config);

        Self {
            fid: info.fid.clone(),
            full_fid: info.full_fid.clone(),

            info: Arc::new(Mutex::new(info)),
            name: service_config.name.clone(),
            state: Arc::new(Mutex::new(ServiceInnerState::new())),
        }
    }

    pub fn as_ood_daemon(&self) -> bool {
        self.info.lock().unwrap().as_ood_daemon()
    }

    pub fn mark_ood_daemon(&mut self, as_ood_daemon: bool) {
        self.info.lock().unwrap().mark_ood_daemon(as_ood_daemon)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn fid(&self) -> &str {
        &self.fid
    }

    // bind成功后才可以调用
    pub fn current(&self) -> PathBuf {
        self.info.lock().unwrap().current()
    }

    pub fn state(&self) -> ServiceState {
        self.state.lock().unwrap().state
    }

    pub fn bind(&mut self, root: &Path) {
        self.info.lock().unwrap().bind(root);
    }

    // bind后，初始化一个service
    pub async fn init(&self) -> BuckyResult<()> {
        assert!(self.state() == ServiceState::STOP);

        let mut need_sync = true;
        match self.load_package() {
            Ok(_) => {
                self.update_state();
                need_sync = false;
            }
            Err(_) => {}
        }

        // 如果本地目录无法加载，或者是空目录，那么尝试同步包
        if need_sync {
            // FIXME 多线程支持
            if let Err(e) = self.sync_package().await {
                let msg = format!(
                    "sync service package failed! service={}, err={}",
                    self.name, e
                );

                error!("{}", msg);
                Err(BuckyError::from(msg))
            } else {
                info!("sync service package success! service={}", self.name);

                Ok(())
            }
        } else {
            Ok(())
        }
    }

    fn start(&self, need_update_state: bool) {
        if self.as_ood_daemon() {
            return;
        }

        if need_update_state {
            self.update_state();
        }
        if self.state() == ServiceState::RUN {
            return;
        }

        self.start_process();
    }

    // 直接开启ood-daemon
    pub fn direct_start_ood_daemon(&self) {
        if self.as_ood_daemon() {
            self.start_process();
        } else {
            unreachable!();
        }
    }

    fn stop(&self, need_update_state: bool) {
        if self.as_ood_daemon() {
            return;
        }

        if need_update_state {
            self.update_state();
        }

        if self.state() == ServiceState::STOP {
            return;
        }

        if self.info.lock().unwrap().get_script("stop").is_none() {
            warn!("stop script is none, service={}", self.name);
            self.kill_process();
        } else {
            self.stop_process_by_cmd();
        }

        if need_update_state {
            self.update_state();
        }
    }

    pub fn sync_state(&self, target_state: ServiceState) {
        if self.as_ood_daemon() {
            return;
        }

        if !OOD_CONTROLLER.is_bind() {
            warn!(
                "ood not bind yet! service={}, state={}",
                self.name, target_state
            );
            return;
        }

        self.update_state();

        if self.state() == target_state {
            debug!(
                "service state match: service={}, state={}",
                self.name, target_state
            );
            return;
        }

        info!(
            "will sync service state: service={}, {} => {}",
            self.name,
            self.state(),
            target_state
        );

        match target_state {
            ServiceState::RUN => {
                self.start(false);
            }
            ServiceState::STOP => {
                self.stop(false);
            }
        }
    }

    pub fn remove(&self) {}

    fn kill_process(&self) {
        if self.as_ood_daemon() {
            return;
        }

        let process = self.state.lock().unwrap().process.take();

        assert!(process.is_some());

        let mut process = process.unwrap();
        match process.kill() {
            Ok(_) => {
                info!("kill service success, name={}", self.name);
            }
            Err(err) => {
                if err.kind() == std::io::ErrorKind::InvalidInput {
                    info!("kill service bus not exists! name={}", self.name);
                } else {
                    error!("kill service got err, name={}, err={}", self.name, err);
                }
            }
        }

        // 需要通过wait来释放进程的一些资源
        match process.wait() {
            Ok(status) => {
                info!(
                    "service exit! service={}, status={}",
                    self.name,
                    status.code().unwrap_or_default()
                );
            }
            Err(e) => {
                error!("service exit error! service={}, err={}", self.name, e);
            }
        }
    }

    fn stop_process_by_cmd(&self) {
        assert!(!self.as_ood_daemon());

        // 如果进程是我们自己拉起的，那么优先尝试使用进程对象来停止
        if self.state.lock().unwrap().process.is_some() {
            self.kill_process();
        }

        // 使用package.cfg里面配置的stop命令来停止
        let stop_script = self.info.lock().unwrap().get_script("stop");
        if stop_script.is_none() {
            warn!("stop script is none, service={}", self.name);

            return;
        }

        let v = stop_script.as_ref().unwrap();
        let mut cmd = self.gen_cmd(&v, false, false).unwrap();
        info!(
            "will stop service by cmd: name={}, cmd={:?}",
            self.name, cmd
        );

        match cmd.spawn() {
            Ok(mut child) => match child.wait() {
                Ok(status) => {
                    info!(
                        "stop cmd complete, service={}, code={}, cmd={}",
                        self.name,
                        status.code().unwrap_or_default(),
                        v,
                    );
                }
                Err(err) => {
                    error!(
                        "wait stop cmd error, service={}, err={}, cmd={}",
                        self.name, err, v
                    );
                }
            },
            Err(err) => {
                error!(
                    "exec stop cmd error! service={}, err={}, cmd={}",
                    self.name, err, v
                );
            }
        }
    }

    fn start_process(&self) {
        // assert!(!self.as_ood_daemon());

        let start_script = self.info.lock().unwrap().get_script("start");
        if start_script.is_none() {
            warn!("start script is none, service={}", self.name);
            return;
        }

        assert!(self.state.lock().unwrap().process.is_none());
        assert_eq!(self.state(), ServiceState::STOP);

        let v = start_script.as_ref().unwrap();

        // 在debug情况下，输出控制台日志到父进程
        #[cfg(not(debug_assertions))]
        let ignore_output = true;

        #[cfg(debug_assertions)]
        let ignore_output = false;

        let mut cmd = self.gen_cmd(&v, true, ignore_output).unwrap();
        match cmd.spawn() {
            Ok(p) => {
                info!("spawn service success, service={}, cmd={}", self.name, v);

                // 保存process句柄
                self.state.lock().unwrap().process = Some(p);
                self.change_state(ServiceState::RUN);
            }
            Err(e) => {
                error!(
                    "spawn service error, service={}, err={}, cmd={}",
                    self.name, e, v
                );
            }
        }
    }

    pub fn update_state(&self) {
        if self.as_ood_daemon() {
            return;
        }

        let mut need_cmd_check = false;
        let mut need_change_state = false;
        {
            let mut state = self.state.lock().unwrap();
            let process = state.process.take();
            if process.is_none() {
                need_cmd_check = true;
            } else {
                let mut process = process.unwrap();
                match process.try_wait() {
                    Ok(Some(status)) => {
                        info!("service exited, name={}, status={}", self.name, status);
                        match process.wait() {
                            Ok(_) => {
                                info!("wait service process complete! name={}", self.name);
                            }
                            Err(e) => {
                                info!("wait service process error! name={}, err={}", self.name, e);
                            }
                        }
                        need_change_state = true;
                    }
                    Ok(None) => {
                        debug!("service running: {}", self.name);
                        assert_eq!(state.state, ServiceState::RUN);

                        // 仍然在运行，需要保留进程object并继续等待
                        state.process = Some(process);
                    }
                    Err(e) => {
                        // 出错的情况下，直接放弃这个进程object，并使用cmd进一步检测
                        error!("update service state error, name={}, err={}", self.name, e);
                        need_cmd_check = true;
                    }
                }
            }
        }

        if need_change_state {
            self.change_state(ServiceState::STOP);
        }

        if need_cmd_check {
            // 如果进程对象不存在，那么尝试通过命令行来检测
            self.check_process_by_cmd();
        }
    }

    // 使用check命令行来检测进程是否存在
    fn check_process_by_cmd(&self) {
        assert!(!self.as_ood_daemon());

        let check_script = self.info.lock().unwrap().get_script("status");
        if check_script.is_none() {
            warn!(
                "check script is none, now will treat as stoped! service={}",
                self.name
            );

            // 默认为停止
            self.change_state(ServiceState::STOP);
            return;
        }

        // 检测进程的退出码，0表示不存在，> 0表示目标进程的pid
        let mut exit_code = 0;

        let v = check_script.as_ref().unwrap();
        let mut cmd = self.gen_cmd(&v, false, true).unwrap();

        // 添加目录匹配检测函数 --fid xxxx
        cmd.args(&["--fid", &self.fid]);

        match cmd.spawn() {
            Ok(mut child) => match child.wait() {
                Ok(status) => {
                    exit_code = status.code().unwrap_or_default();

                    debug!(
                        "check cmd complete, name={}, code={}, cmd={}",
                        self.name, exit_code, v
                    );
                }
                Err(err) => {
                    error!(
                        "wait check cmd error, name={}, err={}, cmd={}",
                        self.name, err, v
                    );
                }
            },
            Err(err) => {
                error!(
                    "exec stop cmd error! name={}, err={}, cmd={}",
                    self.name, err, v
                );
            }
        }

        if exit_code != ProcessStatusCode::NotExists as i32 {
            debug!(
                "check service in running: exit code={}, service={}, fid={}",
                exit_code,
                self.name(),
                self.fid
            );
            self.change_state(ServiceState::RUN);

            if exit_code == ProcessStatusCode::RunningOther as i32 {
                warn!(
                    "service running but fid not match! now will kill process, service={}, fid={}",
                    self.name, self.fid
                );
                self.stop(false);
            } else {
            }
        } else {
            self.change_state(ServiceState::STOP);
        }
    }

    fn change_state(&self, new_state: ServiceState) {
        let mut inner_state = self.state.lock().unwrap();
        if inner_state.state != new_state {
            info!(
                "change service state: {} {} => {}",
                self.name(),
                inner_state.state,
                new_state
            );
            inner_state.state = new_state;
        }
    }

    fn gen_cmd(&self, cmd: &str, detach: bool, ignore_output: bool) -> Option<Command> {
        // let mut parts: Vec<&str> = cmd.split_whitespace().collect();
        let mut parts: Vec<&str> = cyfs_util::ProcessUtil::parse_cmd(cmd);
        assert!(parts.len() > 0);

        let mut cmd = Command::new(parts[0]);
        if parts.len() > 1 {
            parts.remove(0);
            cmd.args(&parts);
        }

        if detach {
            cyfs_util::ProcessUtil::detach(&mut cmd);
        }

        //cmd.detach = true;
        if ignore_output {
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
        }
        cmd.current_dir(&self.info.lock().unwrap().root().unwrap());

        Some(cmd)
    }

    /*
    fn gen_cmd_with_nohup(&self, cmd: &str) -> Option<Command> {
        let mut child = Command::new("nohup");
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        assert!(parts.len() > 0);
        child.args(&parts);
        child.arg("&");

        child.stdout(Stdio::null()).stderr(Stdio::null());
        child.current_dir(&self.root.as_ref().unwrap());

        Some(child)
    }
    */
    pub fn load_package(&self) -> BuckyResult<()> {
        self.info.lock().unwrap().load_package()
    }

    pub async fn sync_package(&self) -> Result<bool, BuckyError> {
        if self.check_package() {
            return Ok(false);
        }

        info!(
            "service local package changed or invalid, now will reload! service={}, fid={}",
            self.name, self.full_fid,
        );

        let file = {
            REPO_MANAGER
                .fetch_service(self.full_fid.as_str())
                .await
                .map_err(|e| {
                    error!(
                        "download service package failed! service={}, fid={}, err={}",
                        self.name, self.full_fid, e
                    );

                    e
                })?
        };

        info!(
            "download service package success! service={}, fid={}",
            self.name, self.full_fid,
        );

        // 拷贝文件前，确保服务已经停止
        self.sync_state(ServiceState::STOP);

        // 加载新的包文件
        self.info.lock().unwrap().load_package_file(&file)
    }

    pub fn check_package(&self) -> bool {
        self.info.lock().unwrap().check_package()
    }
}

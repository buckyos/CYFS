use super::service_info::*;
use crate::config::*;
use crate::repo::REPO_MANAGER;
use cyfs_base::{BuckyError, BuckyResult};
use cyfs_util::process::ProcessStatusCode;
use ood_control::OOD_CONTROLLER;

use cyfs_debug::Mutex;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;

#[derive(Debug)]
struct ServiceInnerState {
    state: ServiceState,

    // 拉起服务进程后的进程对象，如果服务进程先于我们启动，则此处为空
    process: Option<Child>,
}

impl ServiceInnerState {
    fn new() -> Self {
        Self {
            state: ServiceState::Stop,
            process: None,
        }
    }
}

#[derive(Clone)]
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

    pub fn package_local_status(&self) -> ServicePackageLocalState {
        self.info.lock().unwrap().state
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

    pub fn get_script(&self, name: &str) -> Option<String> {
        self.info.lock().unwrap().get_script(name)
    }

    // bind后，初始化一个service
    pub async fn init(&self) -> BuckyResult<()> {
        assert!(self.state() == ServiceState::Stop);

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
                Err(BuckyError::new(e.code(), msg))
            } else {
                info!("sync service package success! service={}", self.name);

                Ok(())
            }
        } else {
            Ok(())
        }
    }

    // start the process and change the state on success!
    fn start(&self) {
        if self.as_ood_daemon() {
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

    // stop process and change the state
    fn stop(&self) {
        if self.as_ood_daemon() {
            return;
        }

        // If the process is launched by ourselves, then give priority to trying the process object to stop
        let process_exists = self.state.lock().unwrap().process.is_some();
        if process_exists {
            self.kill_process();
        }

        self.stop_process_by_cmd();

        self.change_state(ServiceState::Stop);
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

        self.direct_sync_state(target_state);
    }

    pub fn direct_sync_state(&self, target_state: ServiceState) {
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
            ServiceState::Run => {
                self.start();
            }
            ServiceState::Stop => {
                self.stop();
            }
        }
    }

    pub fn remove(&self) {}

    fn kill_process(&self) {
        if self.as_ood_daemon() {
            return;
        }

        let process = self.state.lock().unwrap().process.take();
        if process.is_none() {
            error!(
                "try kill process but process object is none! name={}",
                self.name
            );
            return;
        }

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
            Ok(mut child) => {
                let pid = child.id();

                match child.wait() {
                    Ok(status) => {
                        info!(
                            "stop cmd complete, service={}, code={}, cmd={}, pid={}",
                            self.name,
                            status.code().unwrap_or_default(),
                            v,
                            pid,
                        );
                    }
                    Err(err) => {
                        error!(
                            "wait stop cmd error, service={}, err={}, cmd={}, pid={}",
                            self.name, err, v, pid,
                        );
                    }
                }
            }
            Err(err) => {
                error!(
                    "exec stop cmd error! service={}, err={}, cmd={}",
                    self.name, err, v
                );
            }
        }
    }

    // launch the process and change the state if success!
    fn start_process(&self) {
        // assert!(!self.as_ood_daemon());

        let start_script = self.info.lock().unwrap().get_script("start");
        if start_script.is_none() {
            warn!("start script is none, service={}", self.name);
            return;
        }

        if self.state.lock().unwrap().process.is_some() {
            warn!(
                "start process but state.process is not empty! name={}",
                self.name()
            );
            return;
        }

        // assert_eq!(self.state(), ServiceState::Stop);

        let v = start_script.as_ref().unwrap();

        // 在debug情况下，输出控制台日志到父进程
        #[cfg(not(debug_assertions))]
        let ignore_output = true;

        #[cfg(debug_assertions)]
        let ignore_output = false;

        let mut cmd = self.gen_cmd(&v, true, ignore_output).unwrap();
        match cmd.spawn() {
            Ok(p) => {
                info!(
                    "spawn service success, service={}, cmd={}, pid={}",
                    self.name,
                    v,
                    p.id()
                );

                // 保存process句柄
                self.state.lock().unwrap().process = Some(p);
                self.change_state(ServiceState::Run);
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
        {
            let mut state = self.state.lock().unwrap();
            let process = state.process.take();
            if process.is_none() {
                need_cmd_check = true;
            } else {
                let mut process = process.unwrap();
                let pid = process.id();
                match process.try_wait() {
                    Ok(Some(status)) => {
                        info!(
                            "service exited, name={}, status={}, current state={}, pid={}",
                            self.name, status, state.state, pid,
                        );
                        match process.wait() {
                            Ok(_) => {
                                info!(
                                    "wait service process complete! name={}, pid={}",
                                    self.name, pid
                                );
                            }
                            Err(e) => {
                                info!(
                                    "wait service process error! name={}, pid={}, err={}",
                                    self.name, pid, e
                                );
                            }
                        }
                        state.state = ServiceState::Stop;
                    }
                    Ok(None) => {
                        debug!("service still running: name={}, pid={}", self.name, pid);

                        // 仍然在运行，需要保留进程object并继续等待
                        state.process = Some(process);

                        if state.state != ServiceState::Run {
                            warn!("process object exists and still running but state is not run! name={}, current state={}, pid={}", self.name, state.state, pid);
                            state.state = ServiceState::Run;
                        }
                    }
                    Err(e) => {
                        // 出错的情况下，直接放弃这个进程object，并使用cmd进一步检测
                        error!(
                            "update service state error, name={}, err={}, pid={}",
                            self.name, e, pid
                        );
                        need_cmd_check = true;
                    }
                }
            }
        }

        if need_cmd_check {
            // 如果进程对象不存在，那么尝试通过命令行来检测
            self.update_state_by_cmd();
        }
    }

    pub fn check_status_by_cmd(&self) -> Option<i32> {
        let check_script = self.info.lock().unwrap().get_script("status");
        if check_script.is_none() {
            warn!(
                "check script is none, now will treat as stoped! service={}",
                self.name
            );

            return None;
        }

        // 检测进程的退出码，0表示不存在，> 0表示目标进程的pid
        let mut exit_code = 0;

        let v = check_script.as_ref().unwrap();
        let mut cmd = self.gen_cmd(&v, false, true).unwrap();

        // 添加目录匹配检测函数 --fid xxxx
        cmd.args(&["--fid", &self.fid]);

        match cmd.spawn() {
            Ok(mut child) => {
                let pid = child.id();

                match child.wait() {
                    Ok(status) => {
                        exit_code = status.code().unwrap_or_default();

                        debug!(
                            "check cmd complete, name={}, code={}, cmd={}, pid={}",
                            self.name, exit_code, v, pid,
                        );
                    }
                    Err(err) => {
                        error!(
                            "wait check cmd error, name={}, err={}, cmd={}, pid={}",
                            self.name, err, v, pid,
                        );
                    }
                }
            }
            Err(err) => {
                error!(
                    "exec check cmd error! name={}, err={}, cmd={}",
                    self.name, err, v
                );
            }
        }

        Some(exit_code)
    }

    // use the status cmd to check the process and update the state
    fn update_state_by_cmd(&self) {
        assert!(!self.as_ood_daemon());

        let ret = self.check_status_by_cmd();
        if ret.is_none() {
            // 默认为停止
            self.change_state(ServiceState::Stop);
            return;
        }

        let exit_code = ret.unwrap();
        if exit_code != ProcessStatusCode::NotExists as i32 {
            if ProcessStatusCode::is_running_other(exit_code) {
                warn!(
                    "service running but fid not match! now will kill process, service={}, fid={}",
                    self.name, self.fid
                );

                self.stop();
                self.change_state(ServiceState::Stop);
            } else {
                debug!(
                    "check service in running: exit code={}, service={}, fid={}",
                    exit_code,
                    self.name(),
                    self.fid
                );
                self.change_state(ServiceState::Run);
            }
        } else {
            self.change_state(ServiceState::Stop);
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

        self.info.lock().unwrap().state = ServicePackageLocalState::Downloading;

        let file = {
            REPO_MANAGER
                .fetch_service(self.full_fid.as_str())
                .await
                .map_err(|e| {
                    error!(
                        "download service package failed! service={}, fid={}, err={}",
                        self.name, self.full_fid, e
                    );

                    self.info.lock().unwrap().state = ServicePackageLocalState::NotExists;

                    e
                })?
        };

        info!(
            "download service package success! service={}, fid={}",
            self.name, self.full_fid,
        );

        // Before copying the file, make sure that the service has stopped
        self.sync_state(ServiceState::Stop);

        // Load new package files
        let ret = {
            let mut info = self.info.lock().unwrap();
            let ret = info.load_package_file(&file).map_err(|e| {
                info.state = ServicePackageLocalState::Invalid;
                e
            })?;

            info.state = ServicePackageLocalState::Ready;
            ret
        };

        self.sync_state(ServiceState::Stop);

        Ok(ret)
    }

    pub fn check_package(&self) -> bool {
        self.info.lock().unwrap().check_package()
    }
}

#[cfg(test)]
mod test {
    use std::process::Command;

    #[test]
    fn test_exit_code() {
        // let mut output = Command::new("H:/work/CYFS_PUBLIC/src/target/debug/ood-daemon.exe");
        let mut output = Command::new("/mnt/h/work/CYFS_PUBLIC/src/target/debug/ood-daemon");
        let mut p = output.spawn().unwrap();
        let status = p.wait().unwrap();
        assert_eq!(status.code().unwrap(), -1);
    }
}

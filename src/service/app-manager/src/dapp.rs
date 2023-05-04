use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_util::{get_app_dir};
use log::*;
use serde::Deserialize;
use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus};
use std::str::FromStr;
use std::sync::Mutex;
use std::time::Duration;
use wait_timeout::ChildExt;
use crate::process_util::{run, try_stop_process_by_pid};

const STATUS_CMD_TIME_OUT_IN_SECS: u64 = 15;
const STOP_CMD_TIME_OUT_IN_SECS: u64 = 60;
const START_CMD_TIME_OUT_IN_SECS: u64 = 5 * 60;
pub(crate) const INSTALL_CMD_TIME_OUT_IN_SECS: u64 = 15 * 60;

#[derive(Deserialize, Clone)]
pub struct DAppInfo {
    id: String,
    version: String,
    start: String,
    status: String,
    stop: String,
    install: Vec<String>,
    executable: Vec<String>,
}

pub struct DApp {
    dec_id: String,
    info: DAppInfo,
    work_dir: PathBuf,
    process: Mutex<Option<Child>>,
}

fn get_str(value: &Value, key: &str) -> BuckyResult<String> {
    Ok(value
        .get(key)
        .ok_or(BuckyError::from(BuckyErrorCode::InvalidFormat))?
        .as_str()
        .ok_or(BuckyError::from(BuckyErrorCode::InvalidFormat))?
        .to_owned())
}

impl Drop for DApp {
    fn drop(&mut self) {
        if let Some(child) = self.process.lock().unwrap().as_mut() {
            let id = child.id();
            warn!("dapp {} dropped when child process start! pid {}", &self.dec_id, id);
            if let Err(e) = child.kill() {
                error!("kill child process {} err {}", id, e);
            };
            if let Err(e) = child.wait() {
                error!("wait child process {} err {}", id, e);
            };
        }
    }
}

impl DApp {
    pub fn load_from_app_id(app_id: &str) -> BuckyResult<DApp> {
        let dapp = DApp::load_from(&get_app_dir(&app_id.to_string()))?;
        Ok(dapp)
    }

    // load_from
    // 获取dapp的配置信息
    pub fn load_from(path: &PathBuf) -> BuckyResult<DApp> {
        let package_file = path.join("package.cfg");
        if !package_file.exists() {
            error!("package file {} not exist!", package_file.display());
            return Err(BuckyError::from(BuckyErrorCode::NotFound));
        }

        // 通过上一级目录拿到 decid
        let dec_id = {
            let parent = package_file.parent();
            let dec_id = parent.unwrap().file_name().unwrap();
            let dec_id = dec_id.to_str().unwrap().to_string();
            dec_id
        };

        // open package.cfg 文件
        // 如果json解析失败，就把文件的整个内容，log出来
        let package = File::open(package_file.clone())?;
        let app_info_root = serde_json::from_reader(package);
        if let Err(e) = app_info_root {
            let mut package = File::open(package_file)?;
            let mut content = String::new();
            package.read_to_string(&mut content).map_err(|e| {
                error!("read package.cfg error: {}", e);
                BuckyError::new(
                    BuckyErrorCode::InternalError,
                    format!("read file error: {}", e),
                )
            })?;
            error!(
                "load dapp package.cfg json error, json content: {}",
                content
            );
            return Err(BuckyError::new(
                BuckyErrorCode::JsonError,
                format!("json error: {}", e),
            ));
        }
        let app_info_root = app_info_root.unwrap();
        info!(
            "load dapp package.cfg success, json content: {:?}",
            app_info_root
        );

        // 解析packge.cfg完成，提取关键字段
        let app_info = DApp::parse_info(app_info_root)?;
        if app_info.start.is_empty() {
            let msg = format!("app {} has no start script!", &dec_id);
            warn!("{}", &msg);
        }
        if app_info.status.is_empty() {
            let msg = format!("app {} has no status script!", &dec_id);
            warn!("{}", &msg);
        }
        if app_info.stop.is_empty() {
            let msg = format!("app {} has no stop script!", &dec_id);
            warn!("{}", &msg);
        }
        Ok(DApp {
            dec_id,
            info: app_info,
            work_dir: path.clone(),
            process: Mutex::new(None),
        })
    }

    fn parse_info(root: Value) -> BuckyResult<DAppInfo> {
        let id = get_str(&root, "id")?;

        let version = get_str(&root, "version")?;

        let start = get_str(&root, "start")?;

        let status = get_str(&root, "status")?;

        let stop = get_str(&root, "stop")?;

        let install = match root
            .get("install")
            .ok_or(BuckyError::from(BuckyErrorCode::InvalidFormat))?
        {
            Value::String(str) => Ok(vec![str.to_owned()]),
            Value::Array(array) => {
                let mut install = vec![];
                for value in array {
                    if value.is_string() {
                        install.push(value.as_str().unwrap().to_owned())
                    }
                }
                Ok(install)
            }
            _ => Err(BuckyError::from(BuckyErrorCode::InvalidFormat)),
        }?;
        let mut executable = vec![];
        if let Some(value) = root.get("executable") {
            match value {
                Value::String(str) => executable.push(str.to_owned()),
                Value::Array(array) => {
                    for value in array {
                        if value.is_string() {
                            executable.push(value.as_str().unwrap().to_owned())
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(DAppInfo {
            id,
            version,
            start,
            status,
            stop,
            install,
            executable,
        })
    }

    pub fn get_start_cmd(&self) -> String {
        self.info.start.clone()
    }

    pub fn get_executable_binary(&self) -> BuckyResult<Vec<String>> {
        Ok(self.info.executable.clone())
    }

    fn get_pid_file_path(&self) -> PathBuf {
        cyfs_util::get_cyfs_root_path()
            .join("run")
            .join(format!("app_manager_app_{}", self.dec_id))
    }

    pub fn start(&self) -> BuckyResult<bool> {
        if !self.status()? {
            let child = run(&self.info.start, &self.work_dir, true, None, Some(self.get_pid_file_path().as_path()))?;
            *self.process.lock().unwrap() = Some(child);
            info!(
                "start app:{} {} success!",
                self.dec_id, self.info.id
            );

            return Ok(true);
        }
        Ok(false)
    }

    //time_out == 0 wait forever
    fn run_cmd(
        &self,
        cmd: &str,
        detach: bool,
        stdout: Option<File>,
        time_out: u64,
        record_pid: Option<&Path>
    ) -> BuckyResult<i32> {
        let mut process = run(cmd, &self.work_dir, detach, stdout, record_pid)?;

        let app_id = self.info.id.as_str();

        let wait_exit_status = |status: ExitStatus| match status.code() {
            None => {
                error!("get process code failed, app:{}, cmd:{}", app_id, cmd);
                Err(BuckyError::from(BuckyErrorCode::ExecuteError))
            }
            Some(code) => {
                info!(
                    "get process code success, app:{}, cmd:{}, code:{}",
                    app_id, cmd, code
                );
                Ok(code)
            }
        };

        if time_out == 0 {
            let exit_status = process.wait().map_err(|e| {
                error!(
                    "wait process failed, app:{}, cmd:{}, err:{}",
                    app_id, cmd, e
                );
                e
            })?;
            return wait_exit_status(exit_status);
        }

        let exit_status = process
            .wait_timeout(Duration::from_secs(time_out))
            .map_err(|e| {
                warn!(
                    "wait timeout process failed, app:{}, cmd:{}, err:{}",
                    app_id, cmd, e
                );
                e
            })?;

        match exit_status {
            None => {
                error!(
                    "process not exit after timeout, app:{}, cmd:{}",
                    app_id, cmd
                );

                #[cfg(windows)]
                {
                    let pid = process.id();
                    let _ = Command::new("taskkill").args(["/F", "/T", "/PID", &pid.to_string()])
                        .status();
                }

                let _ = process.kill();
                let _ = process.wait();

                Err(BuckyError::from(BuckyErrorCode::ExecuteError))
            }
            Some(status) => wait_exit_status(status),
        }
    }

    fn status_by_cmd(&self) -> BuckyResult<bool> {
        // 通过命令行判定app运行状态
        let exit_code = self.run_cmd(
            &self.info.status,
            false,
            None,
            STATUS_CMD_TIME_OUT_IN_SECS,
            None,
        )?;
        Ok(exit_code != 0)
    }

    pub fn status(&self) -> BuckyResult<bool> {
        let mut proc = self.process.lock().unwrap();
        if proc.is_none() {
            info!("process obj not exist, check by cmd");
            self.status_by_cmd()
        } else {
            // app是这个进程起的，通过Child对象来判断状态，也能阻止僵尸进程
            info!("process obj exist, try wait");
            match proc.as_mut().unwrap().try_wait() {
                Ok(Some(status)) => {
                    info!("app exited, name={}, status={}", self.info.id, status);

                    let mut process = proc.take().unwrap();
                    match process.wait() {
                        Ok(_) => {
                            info!("wait app process complete! name={}", self.info.id);
                        }
                        Err(e) => {
                            info!("wait app process error! name={}, err={}", self.info.id, e);
                        }
                    }

                    Ok(false)
                }
                Ok(None) => {
                    info!("app running, name={}", self.info.id);
                    Ok(true)
                }
                Err(e) => {
                    error!("update app state error, name={}, err={}", self.info.id, e);

                    self.status_by_cmd()
                }
            }
        }
    }

    pub fn stop(&self) -> BuckyResult<bool> {
        match self.status() {
            Err(e) => {
                warn!("check app status failed, app:{}, err:{}", &self.info.id, e);
                let _ = self._force_stop();
            }
            Ok(is_running) => {
                if is_running {
                    let process = self.process.lock().unwrap().take();
                    if process.is_some() {
                        let mut process = process.unwrap();
                        info!("stop app through child process");
                        match process.kill() {
                            Ok(_) => {
                                info!("kill app success, name={}", &self.info.id);
                            }
                            Err(err) => {
                                if err.kind() == std::io::ErrorKind::InvalidInput {
                                    info!("kill app but not exists! name={}", &self.info.id);
                                } else {
                                    error!("kill app got err, name={}, err={}", &self.info.id, err);
                                }
                            }
                        }

                        // 需要通过wait来释放进程的一些资源
                        match process.wait() {
                            Ok(status) => {
                                info!(
                                    "app exit! service={}, status={}",
                                    &self.info.id,
                                    status.code().unwrap_or_default()
                                );
                            }
                            Err(e) => {
                                error!("app exit error! service={}, err={}", &self.info.id, e);
                            }
                        }
                        return Ok(true);
                    } else {
                        info!("stop app through cmd");

                        match self.run_cmd(
                            &self.info.stop,
                            false,
                            None,
                            STOP_CMD_TIME_OUT_IN_SECS,
                            None,
                        ) {
                            Ok(code) => {
                                if code != 0 {
                                    let _ = self._force_stop();
                                }
                            }
                            Err(e) => {
                                error!("kill app by cmd failed, err:{}", e);
                                let _ = self._force_stop();
                            }
                        }
                    }
                } else {
                    let _ = self._force_stop();
                }
            }
        }

        Ok(false)
    }

    // _force_stop
    // system kill app by pid
    // appmanager 通过start记录的pid去兜底删除应用
    fn _force_stop(&self) -> BuckyResult<()> {
        try_stop_process_by_pid(self.get_pid_file_path().as_path(), Some(self.work_dir.as_path()))
    }

    // 这里做DecApp被安装后，执行前，根据配置文件需要做的预配置
    pub fn prepare(&self) -> BuckyResult<()> {
        // 非windows下，设置executable对应的文件为可执行
        #[cfg(not(windows))]
        {
            for path in &self.info.executable {
                let cmd = format!("chmod +x \"{}\"", path);
                // 就算执行不成功，也可以让开发者打包的时候就设置好，这里不成功不算错
                let _ = self.run_cmd(
                    &cmd,
                    false,
                    None,
                    0,
                    None
                );
            }
        }
        Ok(())
    }

    pub fn get_install_cmd(&self) -> Vec<String> {
        self.info.install.clone()
    }

    pub fn install(&self, pid_path: Option<&Path>) -> BuckyResult<bool> {
        let mut cmd_index = 0;
        for cmd in &self.info.install {
            let log_file = self.work_dir.join(format!("install_{}.log", cmd_index));

            match self.run_cmd(
                cmd,
                false,
                File::create(log_file).ok(),
                INSTALL_CMD_TIME_OUT_IN_SECS,
                pid_path
            ) {
                Err(e) => {
                    error!("run app:{} install cmd {} err {}", &self.info.id, cmd, e);
                    return Err(e);
                }
                Ok(code) => {
                    if code != 0 {
                        error!(
                            "run app:{} install cmd {} exit code: {}",
                            &self.info.id, cmd, code
                        );
                        return Err(BuckyError::from(BuckyErrorCode::ExecuteError));
                    }
                    cmd_index += 1;
                }
            };
        }
        Ok(true)
    }
}

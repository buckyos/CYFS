use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_util::{ProcessUtil, get_app_dir};
use log::*;
use serde::Deserialize;
use serde_json::Value;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};

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
    process: Option<Child>,
}

fn get_str(value: &Value, key: &str) -> BuckyResult<String> {
    Ok(value
        .get(key)
        .ok_or(BuckyError::from(BuckyErrorCode::InvalidFormat))?
        .as_str()
        .ok_or(BuckyError::from(BuckyErrorCode::InvalidFormat))?
        .to_owned())
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
        Ok(DApp {
            dec_id: dec_id,
            info: app_info,
            work_dir: path.clone(),
            process: None,
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

    fn run(cmd: &str, dir: &Path, detach: bool, stdout: Option<File>) -> BuckyResult<Child> {
        let args: Vec<&str> = ProcessUtil::parse_cmd(cmd);
        if args.len() == 0 {
            return Err(BuckyError::from(BuckyErrorCode::InvalidData));
        }
        info!("run cmd {} in {}", cmd, dir.display());
        let program = which::which(args[0]).unwrap_or_else(|_| dir.join(args[0]));
        let mut command = Command::new(program);
        command.args(&args[1..]).current_dir(dir);
        if let Some(out) = stdout {
            command.stdout(out);
        }
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }

        if detach {
            ProcessUtil::detach(&mut command);
        }

        match command.spawn() {
            Ok(p) => Ok(p),
            Err(e) => {
                error!(
                    "spawn app failed! cmd {}, dir {}, err {}",
                    cmd,
                    dir.display(),
                    e
                );
                Err(BuckyError::from(BuckyErrorCode::ExecuteError))
            }
        }
    }

    pub fn get_start_cmd(&mut self) -> BuckyResult<&str> {
        Ok(&self.info.start)
    }

    pub fn get_executable_binary(&self) -> BuckyResult<Vec<String>> {
        Ok(self.info.executable.clone())
    }

    fn get_pid_file_path(&self) -> BuckyResult<PathBuf> {
        let pid_file = cyfs_util::get_cyfs_root_path()
            .join("run")
            .join(format!("app_manager_app_{}", self.dec_id));
        Ok(pid_file)
    }

    fn get_pid(&self) -> BuckyResult<String> {
        let lock_file = self.get_pid_file_path()?;
        if !lock_file.is_file() {
            return Err(BuckyError::new(BuckyErrorCode::NotFound, "no pid file"));
        }
        let result = std::fs::read_to_string(lock_file).unwrap();
        Ok(result)
    }

    pub fn start(&mut self) -> BuckyResult<bool> {
        if !self.status()? {
            let child = DApp::run(&self.info.start, &self.work_dir, true, None)?;

            // mark pid
            let id = child.id();
            let lock_file = self.get_pid_file_path()?;
            let buf = format!("{}", id).into_bytes();
            std::fs::write(lock_file, &buf).map_err(|e| {
                error!(
                    "app[{}]{} write lock file failed! err {}",
                    self.dec_id, self.info.id, e
                );
                BuckyError::from(BuckyErrorCode::ExecuteError)
            })?;
            info!(
                "start app {} {} success! and write pid {:?}",
                self.dec_id, self.info.id, id
            );

            self.process = Some(child);

            return Ok(true);
        }
        Ok(false)
    }

    fn status_by_cmd(&self) -> BuckyResult<bool> {
        // 通过命令行判定app运行状态
        let mut ret = DApp::run(&self.info.status, &self.work_dir, false, None)?;
        return match ret.wait()?.code() {
            None => {
                error!("app {} get no ret", self.info.id);
                Err(BuckyError::from(BuckyErrorCode::ExecuteError))
            }
            Some(code) => Ok(code != 0),
        };
    }

    pub fn status(&mut self) -> BuckyResult<bool> {
        if self.process.is_none() {
            info!("process obj not exist, check by cmd");
            self.status_by_cmd()
        } else {
            // app是这个进程起的，通过Child对象来判断状态，也能阻止僵尸进程
            info!("process obj exist, try wait");
            match self.process.as_mut().unwrap().try_wait() {
                Ok(Some(status)) => {
                    info!("app exited, name={}, status={}", self.info.id, status);

                    let mut process = std::mem::replace(&mut self.process, None).unwrap();
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

    pub fn stop(&mut self) -> BuckyResult<bool> {
        if self.status()? {
            let process = self.process.take();
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
                let result = match DApp::run(&self.info.stop, &self.work_dir, false, None) {
                    Ok(mut child) => Ok(child.wait()?.code().unwrap_or(0) == 0),
                    Err(e) => Err(e),
                };

                if !result? {
                    let _ = self._force_stop();
                }
            }
        } else {
            // info!("app:{} inner status error, try to stop by pid", self.info.id);
            let _ = self._force_stop();
        }

        Ok(false)
    }

    // _force_stop
    // system kill app by pid
    // appmanager 通过start记录的pid去兜底删除应用
    fn _force_stop(&self) -> BuckyResult<()> {
        let pid = self.get_pid();
        if pid.is_err() {
            return Ok(());
        }

        let pid = pid.unwrap();
        info!(
            "stop app[{}] by inner cmd failed, try to force kill by pid {}",
            &self.info.id, pid
        );
        #[cfg(windows)]
        {
            Command::new("taskkill")
                .arg("/F")
                .arg("/T")
                .arg("/PID")
                .arg(&pid)
                .spawn()
                .map_err(|e| {
                    error!("kill app {} failed! err {}", pid, e);
                    BuckyError::from(BuckyErrorCode::ExecuteError)
                })?;
        }
        #[cfg(not(windows))]
        {
            Command::new("kill")
                .arg("-9")
                .arg(&pid)
                .spawn()
                .map_err(|e| {
                    error!("kill app {} failed! err {}", pid, e);
                    BuckyError::from(BuckyErrorCode::ExecuteError)
                })?;
        }

        let lock_file = self.get_pid_file_path()?;
        let _ = std::fs::remove_file(lock_file);
        Ok(())
    }

    pub fn install(&self) -> BuckyResult<bool> {
        // 非windows下，设置executable对应的文件为可执行
        #[cfg(not(windows))]
        {
            for path in &self.info.executable {
                let cmd = format!("chmod +x \"{}\"", path);
                // 就算执行不成功，也可以让开发者打包的时候就设置好，这里不成功不算错
                if let Ok(mut child) = DApp::run(&cmd, &self.work_dir, false, None) {
                    let _ = child.wait();
                }
            }
        }
        let mut cmd_index = 0;
        for cmd in &self.info.install {
            let log_file = self.work_dir.join(format!("install_{}.log", cmd_index));
            let file = std::fs::File::create(log_file).ok();
            let mut child = DApp::run(cmd, &self.work_dir, false, file)?;
            let exit_code = child.wait()?;
            if !exit_code.success() {
                error!(
                    "run app {} install cmd {} err {}",
                    &self.info.id,
                    cmd,
                    exit_code.code().unwrap_or(-1)
                );
                return Err(BuckyError::from(BuckyErrorCode::ExecuteError));
            }
            cmd_index += 1;
        }
        Ok(true)
    }
}

use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::str::FromStr;
use sysinfo::{Pid, ProcessExt, ProcessRefreshKind, RefreshKind, SystemExt};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use log::*;
use cyfs_core::DecAppId;
use cyfs_util::ProcessUtil;

pub fn run(cmd: &str, work_dir: &Path, detach: bool, stdout: Option<File>, record_pid: Option<&Path>) -> BuckyResult<Child> {
    let args: Vec<&str> = ProcessUtil::parse_cmd(cmd);
    if args.len() == 0 {
        error!("parse cmd {} failed, cmd empty?", cmd);
        return Err(BuckyError::from(BuckyErrorCode::InvalidData));
    }
    info!("run cmd {} in {}", cmd, work_dir.display());
    let program = which::which(args[0]).unwrap_or_else(|_| work_dir.join(args[0]));
    info!("program full path: {}", program.display());
    let mut command = Command::new(program);
    command.args(&args[1..]).current_dir(work_dir);
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
        Ok(p) => {
            if let Some(path) = record_pid {
                info!("write process pid {} to {}", p.id(), path.display());
                let _ = std::fs::write(path, p.id().to_string().as_bytes());
            }
            Ok(p)
        },
        Err(e) => {
            error!("spawn app failed! cmd {}, dir {}, err {}", cmd, work_dir.display(), e);
            Err(BuckyError::from(BuckyErrorCode::ExecuteError))
        }
    }
}

pub fn try_stop_process_by_pid(pid_path: &Path, match_work_dir: Option<&Path>) -> BuckyResult<()> {
    if pid_path.is_file() {
        let pid = std::fs::read_to_string(pid_path)?;
        let info = sysinfo::System::new_with_specifics(RefreshKind::new().with_processes(ProcessRefreshKind::new()));
        if let Some(process) = info.process(Pid::from_str(&pid)?) {
            if let Some(path) = match_work_dir {
                if process.cwd() != path {
                    warn!("pid {} work dir mismatch! except {}, actual {}. not kill", &pid, path.display(), process.cwd().display());
                    return Ok(());
                }
            }
            info!("try to force kill process by pid {}", pid);
            let cmd;
            #[cfg(windows)]
            {
                cmd = format!("taskkill /F /T /PID {}", &pid);
            }
            #[cfg(not(windows))]
            {
                cmd = format!("kill -9 {}", &pid);
            }
            run(&cmd, &Path::new("."), false, None, None)?.wait()?;
        }
    } else {
        info!("not found or not file: pid path {}", pid_path.display());
    }

    if pid_path.is_dir() {
        if let Ok(_) = std::fs::remove_dir_all(pid_path) {
            info!("delete pid path {}?", pid_path.display());
        }
    } else {
        if let Ok(_) = std::fs::remove_file(pid_path) {
            info!("delete pid file {}", pid_path.display());
        }
    }

    Ok(())
}

pub fn get_install_pid_file_path(id: &DecAppId) -> PathBuf {
    cyfs_util::get_cyfs_root_path()
        .join("run")
        .join(format!("app_manager_app_install_{}", id))
}
#[cfg(unix)]
pub mod daemon {
    use cyfs_base::BuckyError;

    use std::process::{exit, Command};

    use nix::{
        sys::wait::{waitpid, WaitStatus},
        unistd::{fork, setsid, ForkResult},
    };

    pub fn launch_as_daemon(cmd_line: &str) -> Result<(), BuckyError> {
        let ret = unsafe { fork() }.map_err(|e| {
            let msg = format!("fork error: {}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        match ret {
            ForkResult::Parent { child } => {
                info!("fork child as daemon success: {}", child);

                match waitpid(child, None) {
                    Ok(status) => {
                        info!("fork child exit: {} {:?}", child, status);
                        if let WaitStatus::Exited(_pid, code) = status {
                            if code == 0 {
                                return Ok(());
                            }
                        }

                        let msg = format!("fork child but wait error: {}, {:?}", child, status);
                        error!("{}", msg);

                        Err(BuckyError::from(msg))
                    }
                    Err(e) => {
                        let msg = format!("fork child wait error: {} {}", child, e);
                        error!("{}", msg);

                        Err(BuckyError::from(msg))
                    }
                }
            }

            ForkResult::Child => {
                match setsid() {
                    Ok(sid) => {
                        info!("new sid: {}", sid);
                    }
                    Err(e) => {
                        error!("setsid error: {}", e);
                        exit(1);
                    }
                }

                let mut parts: Vec<&str> = crate::ProcessUtil::parse_cmd(cmd_line);
                assert!(parts.len() > 0);

                let mut cmd = Command::new(parts[0]);
                if parts.len() > 1 {
                    parts.remove(0);
                    cmd.args(&parts);
                }

                let code = match cmd.spawn() {
                    Ok(_) => {
                        info!("spawn daemon success!");
                        0
                    }
                    Err(err) => {
                        error!("spawn daemon error: {}", err);
                        1
                    }
                };

                exit(code);
            }
        }
    }
}

#[cfg(windows)]
pub mod daemon {
    use cyfs_base::BuckyError;
    use std::process::{Command, Stdio};

    pub fn launch_as_daemon(cmd_line: &str) -> Result<(), BuckyError> {
        let mut parts: Vec<&str> = crate::ProcessUtil::parse_cmd(cmd_line);
        assert!(parts.len() > 0);

        let mut cmd = Command::new(parts[0]);
        if parts.len() > 1 {
            parts.remove(0);
            cmd.args(&parts);
        }

        crate::ProcessUtil::detach(&mut cmd);

        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        match cmd.spawn() {
            Ok(child) => {
                info!("spawn as daemon success: {}, pid={}", cmd_line, child.id());

                Ok(())
            }
            Err(err) => {
                let msg = format!("spawn as daemon error: {} {}", cmd_line, err);
                error!("{}", msg);

                Err(BuckyError::from(msg))
            }
        }
    }
}

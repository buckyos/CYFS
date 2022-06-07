#[cfg(unix)]
pub mod daemon {
    use cyfs_base::BuckyError;

    use std::process::{exit, Command};

    use nix::{
        sys::wait::waitpid,
        unistd::{fork, setsid, ForkResult},
    };

    pub fn launch_as_daemon(cmd_line: &str) -> Result<(), BuckyError> {
        let ret = unsafe {fork()}.map_err(|e| {
            let msg = format!("fork error: {}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })? ;

        match ret {
            ForkResult::Parent { child } => {
                info!("fork child as daemon success: {}", child);

                match waitpid(child, None) {
                    Ok(status) => {
                        info!("fork child exit: {} {:?}", child, status);
                    }
                    Err(e) => {
                        error!("fork child wait error: {} {}", child, e);
                    }
                }

                Ok(())
            }

            ForkResult::Child => {
                match setsid() {
                    Ok(sid) => {
                        info!("new sid: {}", sid);
                    }
                    Err(e) => {
                        error!("setsid error: {}", e);
                    }
                }

                let mut parts: Vec<&str> = crate::ProcessUtil::parse_cmd(cmd_line);
                assert!(parts.len() > 0);

                let mut cmd = Command::new(parts[0]);
                if parts.len() > 1 {
                    parts.remove(0);
                    cmd.args(&parts);
                }

                match cmd.spawn() {
                    Ok(_) => {
                        info!("spawn daemon success!");
                    }
                    Err(err) => {
                        error!("spawn daemon error: {}", err);
                    }
                }

                exit(0);
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
            Ok(_) => {
                info!("spawn as daemon success: {}", cmd_line);

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

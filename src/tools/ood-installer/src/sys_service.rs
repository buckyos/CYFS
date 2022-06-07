use os_type::OSType;
use std::process::Command;

use cyfs_base::{BuckyError, BuckyResult};

pub struct SysService {}

impl SysService {
    pub fn init() -> BuckyResult<()> {
        let cmd_line;
        let current_os_type = os_type::current_platform().os_type;
        info!("current system: {:?}", current_os_type);

        // https://blog.frd.mn/how-to-set-up-proper-startstop-services-ubuntu-debian-mac-windows/
        match current_os_type {
            OSType::Ubuntu | OSType::Debian | OSType::Deepin => {
                cmd_line = "update-rc.d ood-daemon defaults";
            }
            OSType::CentOS => {
                cmd_line = "chkconfig ood-daemon on";
            }
            _ => {
                let msg = format!(
                    "sys service not support on this system! platform={:?}",
                    current_os_type
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        return Self::exec_cmd(cmd_line);
    }

    fn exec_cmd(cmd_line: &str) -> BuckyResult<()> {
        let mut parts: Vec<&str> = cmd_line.split_whitespace().collect();
        assert!(parts.len() > 0);

        let mut cmd = Command::new(parts[0]);
        if parts.len() > 1 {
            parts.remove(0);
            cmd.args(&parts);
        }

        match cmd.spawn() {
            Ok(mut child) => match child.wait() {
                Ok(status) => {
                    if status.success() {
                        info!("exec {} success!", cmd_line);
                        Ok(())
                    } else {
                        let msg = format!("exec {} got failed! code={:?}", cmd_line, status.code());
                        error!("{}", msg);

                        return Err(BuckyError::from(msg));
                    }
                }
                Err(e) => {
                    let msg = format!("wait {} failed! err={}", cmd_line, e);
                    error!("{}", msg);

                    return Err(BuckyError::from(msg));
                }
            },
            Err(e) => {
                let msg = format!("launch {} failed! err={}", cmd_line, e);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }
    }
}

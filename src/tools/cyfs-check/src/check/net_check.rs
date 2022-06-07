use async_trait::async_trait;
use crate::{CheckCore, CheckType};
use log::*;

pub struct NetCheck {}

#[async_trait]
impl CheckCore for NetCheck {
    async fn check(&self, _: CheckType) -> bool {
        match cyfs_util::get_if_addrs() {
            Ok(ifs) => {
                if ifs.len() == 0 {
                    error!("no valid interface");
                    false
                } else {
                    for interface in ifs {
                        info!("interface {}:{}\n\t{}\n\tflags:{}", interface.name, interface.description, interface.addr.ip().to_string(), interface.ifa_flags);
                    }
                    true
                }
            }
            Err(e) => {
                error!("get net interface err {}", e);
                false
            }
        }
    }

    fn name(&self) -> &str {
        "Net Check"
    }
}
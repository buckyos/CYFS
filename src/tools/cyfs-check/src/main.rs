mod check;
mod check_logger;

use clap::{App, Arg};
use log::*;
use async_trait::async_trait;

use crate::check::*;
use std::fmt::Formatter;
use crate::check_logger::CheckLogger;

#[derive(PartialEq, Eq, Copy, Clone)]
pub enum CheckType {
    OOD,
    Runtime
}

impl std::fmt::Display for CheckType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CheckType::OOD => {f.write_str("OOD")}
            CheckType::Runtime => {f.write_str("Runtime")}
        }
    }
}

#[async_trait]
pub trait CheckCore {
    // check返回false，表示过程中出现了环境错误，在这里就停止后续check了
    async fn check(&self, check_type: CheckType) -> bool;
    fn name(&self) -> &str;
}

#[async_std::main]
async fn main() {
    let matches = App::new("cyfs-check").version(cyfs_base::get_version()).about("check cyfs env")
        .arg(Arg::with_name("save").short("s").long("save").default_value("./cyfs-check-result.txt"))
        .arg(Arg::with_name("no-ood").long("no-ood"))
        .arg(Arg::with_name("no-runtime").long("no-runtime"))
        .get_matches();

    let logger = if matches.occurrences_of("save") != 0 {
        CheckLogger::new(matches.value_of("save"))
    } else {
        CheckLogger::new(None)
    };

    logger.start().unwrap();

    let checks: Vec<Box<dyn CheckCore>> = vec![
        Box::new(CyfsBaseCheck {}),
        Box::new(CyfsCoreCheck {}),
        Box::new(CyfsDescCheck {})
    ];

    // 先检测网卡
    trace!("===================== Net Interface Info ============================");
    debug!("check net interface");
    let net_check = NetCheck {};
    let ret = net_check.check(CheckType::OOD).await;
    trace!("===================== Net Interface Info End ========================");
    if !ret {
        return;
    }

    // 先检测OOD
    if !matches.is_present("no-ood") {
        debug!("start OOD check");
        for check in &checks {
            trace!("************************** OOD {} ********************************", check.name());
            debug!("start {} {}", "OOD", check.name());
            let ret = check.check(CheckType::OOD).await;
            if !ret {
                error!("{} failed", check.name());
            }
            trace!("************************** OOD {} End ********************************", check.name());
            trace!("");
            if !ret {
                break;
            }
        }
    }

    if !matches.is_present("no-runtime") {
        if let Some(data_dir) = dirs::data_dir() {
            let runtime_root = data_dir.join("cyfs");
            cyfs_util::bind_cyfs_root_path(runtime_root);
        } else {
            error!("get user data dir failed.");
            return;
        }
        debug!("start Runtime check");
        for check in &checks {
            trace!("************************** Runtime {} ********************************", check.name());
            debug!("start {} {}", "Runtime", check.name());
            let ret = check.check(CheckType::Runtime).await;
            if !ret {
                error!("{} failed", check.name());
            }
            trace!("************************** Runtime {} End ********************************", check.name());
            trace!("");
            if !ret {
                break;
            }
        }
    }
}

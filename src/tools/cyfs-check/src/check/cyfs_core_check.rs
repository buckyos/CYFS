use crate::{CheckCore, CheckType};
use async_trait::async_trait;
use cyfs_base::{FileDecoder, NamedObject, ObjectDesc};
use log::*;

pub struct CyfsCoreCheck {}

impl CyfsCoreCheck {
    pub fn new() -> Self {
        CyfsCoreCheck {}
    }
}

#[async_trait]
impl CheckCore for CyfsCoreCheck {
    async fn check(&self, check_type: CheckType) -> bool {
        let desc_path = cyfs_util::get_service_config_dir("desc");
        let sn_path = desc_path.join("sn.desc");
        if sn_path.exists() {
            warn!(
                "{} user defined SN exists at {}",
                check_type,
                sn_path.display()
            );
            match cyfs_base::Device::decode_from_file(&sn_path, &mut vec![]) {
                Ok((sn, _)) => {
                    info!(
                        "{} user defined SN: {}",
                        check_type,
                        sn.desc().calculate_id()
                    );
                }
                Err(e) => {
                    error!("{} decode user defined SN Err {}", check_type, e);
                }
            }
        }

        let pn_path = desc_path.join("pn.desc");
        if pn_path.exists() {
            warn!(
                "{} user defined PN exists at {}",
                check_type,
                sn_path.display()
            );
            match cyfs_base::Device::decode_from_file(&sn_path, &mut vec![]) {
                Ok((sn, _)) => {
                    info!(
                        "{} user defined PN: {}",
                        check_type,
                        sn.desc().calculate_id()
                    );
                }
                Err(e) => {
                    error!("{} decode user defined PN Err {}", check_type, e);
                }
            }
        }

        let debug_config = cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("debug.toml");
        if debug_config.exists() {
            info!(
                "{} log config exists at {}",
                check_type,
                debug_config.display()
            );
            debug!("{} log config will print to file", check_type);
            match std::fs::read_to_string(&debug_config) {
                Ok(config) => {
                    trace!("{}", config);
                    trace!("");
                }
                Err(e) => {
                    error!("read from {} err {}!", debug_config.display(), e);
                }
            }
        } else {
            warn!(
                "{} log config not exists, it will at {}",
                check_type,
                debug_config.display()
            );
        }

        let acl_config = cyfs_util::get_service_config_dir("acl").join("acl.toml");
        if acl_config.exists() {
            info!(
                "{} acl config exists at {}",
                check_type,
                acl_config.display()
            );
            debug!("{} acl config will print to file", check_type);
            match std::fs::read_to_string(&acl_config) {
                Ok(config) => {
                    trace!("{}", config);
                    trace!("");
                }
                Err(e) => {
                    error!("read from {} err {}!", acl_config.display(), e);
                }
            }
        } else {
            warn!(
                "{} acl config not exists, it will at {}",
                check_type,
                acl_config.display()
            );
        }

        let handler_config = cyfs_util::get_service_config_dir("handler").join("handler.toml");
        if handler_config.exists() {
            info!(
                "{} handler config exists at {}",
                check_type,
                handler_config.display()
            );
            debug!("{} handler config will print to file", check_type);
            match std::fs::read_to_string(&handler_config) {
                Ok(config) => {
                    trace!("{}", config);
                    trace!("");
                }
                Err(e) => {
                    error!("read from {} err {}!", handler_config.display(), e);
                }
            }
        } else {
            warn!(
                "{} handler config not exists, it will at {}",
                check_type,
                handler_config.display()
            );
        }

        // runtime不检测剩下的cyfs_repo, app_repo等文件
        if check_type == CheckType::Runtime {
            return true;
        }

        let cyfs_repo_path = desc_path.join("cyfs_repo.desc");
        if cyfs_repo_path.exists() {
            warn!("CYFS Repo exists at {}", cyfs_repo_path.display());
            match cyfs_base::AnyNamedObject::decode_from_file(&cyfs_repo_path, &mut vec![]) {
                Ok((obj, _)) => {
                    info!("CYFS Repo Id: {}", obj.calculate_id());
                }
                Err(e) => {
                    error!("decode CYFS Repo Err {}", e);
                }
            }
        } else {
            error!(
                "cannot find cyfs repo! it shoule be at {}",
                cyfs_repo_path.display()
            );
        }

        let app_repo_path = desc_path.join("app_repo.desc");
        if app_repo_path.exists() {
            warn!(
                "CYFS Default App Repo exists at {}",
                app_repo_path.display()
            );
            match cyfs_base::AnyNamedObject::decode_from_file(&app_repo_path, &mut vec![]) {
                Ok((obj, _)) => {
                    info!("CYFS Default App Repo Id: {}", obj.calculate_id());
                }
                Err(e) => {
                    error!("decode CYFS Default App Repo Err {}", e);
                }
            }
        } else {
            warn!(
                "cannot find CYFS Default App Repo! it shoule be at {}",
                app_repo_path.display()
            );
        }

        true
    }

    fn name(&self) -> &str {
        "Core Check"
    }
}

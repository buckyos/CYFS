use cyfs_base::*;
use cyfs_core::{DecAppId, get_system_dec_app};
use cyfs_lib::*;
use log::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpecifiedGroup {
    // device/device's owner(as zone id), None for any zone
    pub dec_id: Option<ObjectId>,
    pub zone: Option<ObjectId>,
    // Choose one between zone and zone_category
    pub zone_category: Option<DeviceZoneCategory>,
    pub access: AccessPermissions,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AclConfig {
    access: Option<HashMap<String, AccessString>>,
    specified: Option<HashMap<String, SpecifiedGroup>>,
    link: Option<HashMap<String, String>>,
    config: Option<HashMap<String, String>>,
}

pub type AclConfigs = HashMap<String, AclConfig>;

pub struct AppAclUtil {}

impl AppAclUtil {
    pub fn load_from_file(app_id: &DecAppId, acl_file: &PathBuf) -> BuckyResult<AclConfigs> {
        let contents = std::fs::read_to_string(acl_file).map_err(|e| {
            let msg = format!("read acl config failed! app:{}, err:{}", app_id, e);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!("read acl config, app:{}, {}", app_id, contents);

        let acl_config: AclConfigs = toml::from_str(&contents).map_err(|e| {
            let msg = format!("parse acl config failed! app:{}, err:{}", app_id, e);
            warn!("{}", msg);

            BuckyError::new(BuckyErrorCode::ParseError, msg)
        })?;

        Ok(acl_config)
    }

    pub async fn apply_acl(
        app_id: &DecAppId,
        stack: &SharedCyfsStack,
        acl_config: AclConfigs,
    ) -> BuckyResult<()> {
        let app_obj_id = app_id.object_id();
        for (id, config) in acl_config {
            if id == "self" {
                let stub = stack.root_state_meta_stub(None, Some(app_obj_id.clone()));
                if let Some(link) = config.link {
                    for (target, source) in link {
                        let ret = stub.add_link(&target, &source).await;
                        info!(
                            "[ACL] add self.link. app:{}, target:{}, source:{}, ret:{:?}",
                            app_id, target, source, ret
                        );
                        //TODO: what to do if failed?
                        /*if let Err(e) = ret {
                            warn!(
                                "[ACL] add self.link failed. app:{}, target:{}, source:{}, err:{}",
                                app_id, target, source, e
                            );
                        }*/
                    }
                }

                if let Some(access) = config.access {
                    for (path, access) in access {
                        let ret = stub
                            .add_access(GlobalStatePathAccessItem {
                                path: path.to_owned(),
                                access: GlobalStatePathGroupAccess::Default(access.value()),
                            })
                            .await;
                        info!(
                            "[ACL] add self.access. app:{}, path:{}, access:{:?}, ret:{:?}",
                            app_id, path, access, ret
                        );
                        //TODO: what to do if failed?
                        /*if let Err(e) = ret {
                            warn!(
                                "[ACL] add self.access failed. app:{}, path:{}, access:{:?}, err:{}",
                                app_id, path, access, e
                            );
                        }*/
                    }
                }

                if let Some(specified) = config.specified {
                    for (path, access) in specified {
                        let group = GlobalStatePathSpecifiedGroup {
                            zone: access.zone,
                            zone_category: access.zone_category,
                            dec: access.dec_id,
                            access: access.access as u8,
                        };
                        let ret = stub
                            .add_access(GlobalStatePathAccessItem {
                                path: path.to_owned(),
                                access: GlobalStatePathGroupAccess::Specified(group),
                            })
                            .await;
                        info!(
                            "[ACL] add self.specified. app:{}, path:{}, specified:{:?}, ret:{:?}",
                            app_id, path, access, ret
                        );
                    }
                }
            }else {
                let dec_id = if id == "system" {
                    Ok(get_system_dec_app().clone())
                } else {
                    ObjectId::from_str(&id).map_err(|e| {
                        error!("acl parse dec id {} err {}", id, e);
                        e
                    })
                }?;
                let stub = stack.root_state_meta_stub(None, Some(dec_id.clone()));

                if let Some(specified) = config.specified {
                    for (path, access) in specified {
                        if let Some(dec) = &access.dec_id {
                            if !dec.eq(app_obj_id) {
                                error!(
                                    "[ACL] config {}.specified for others, app:{}, other dec:{}, path:{}, reject!",
                                    id, app_id, dec, path
                                );
                                continue;
                            }
                        }
                        let group = GlobalStatePathSpecifiedGroup {
                            zone: access.zone,
                            zone_category: access.zone_category,
                            dec: Some(app_obj_id.clone()),
                            access: access.access as u8,
                        };
                        let ret = stub
                            .add_access(GlobalStatePathAccessItem {
                                path: path.to_owned(),
                                access: GlobalStatePathGroupAccess::Specified(group),
                            })
                            .await;
                        info!(
                            "[ACL] add {}.specified. app:{}, path:{}, specified:{:?}, ret:{:?}",
                            id, app_id, path, access, ret
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

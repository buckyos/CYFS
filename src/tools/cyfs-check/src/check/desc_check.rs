use crate::{CheckCore, CheckType};
use async_trait::async_trait;
use cyfs_base::{Device, FileDecoder, NamedObject, ObjectDesc, ObjectId, OwnerObjectDesc, People};
use cyfs_base_meta::SavedMetaObject;
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use log::*;

pub struct CyfsDescCheck {}

impl CyfsDescCheck {
    pub fn new() -> Self {
        CyfsDescCheck {}
    }
}

async fn check_ood_list(
    check_type: CheckType,
    owner: &People,
    cur_device: &ObjectId,
    meta_client: &MetaClient,
) {
    let owner_id = owner.desc().calculate_id();
    // 这里先假定owner一定是people先，检查owner的ood_list
    if owner.ood_list().len() == 0 {
        warn!(
            "{} owner {} have empty ood list, not bind ood yet?",
            check_type, owner_id
        );
    } else {
        let ood = &owner.ood_list()[0];
        info!("{} owner {} have ood {}", check_type, owner_id, ood);

        if let Err(e) = meta_client.get_desc(ood.object_id()).await {
            error!(
                "OOD {} not in {} meta chain! {}",
                &ood,
                MetaMinerTarget::default(),
                e,
            );
        } else {
            info!("OOD {} in {} meta chain", &ood, MetaMinerTarget::default());
        }

        if check_type == CheckType::OOD {
            if !ood.object_id().eq(cur_device) {
                error!(
                    "{} owner {}`s ood list not contain this ood {}",
                    check_type, owner_id, &cur_device
                );
            }
        }
    }
}

async fn check_owner(
    check_type: CheckType,
    meta_client: &MetaClient,
    device: &Device,
) -> Option<SavedMetaObject> {
    if let Some(owner) = device.desc().owner() {
        info!("{} device has owner {}", check_type, owner);
        match meta_client.get_desc(owner).await {
            Ok(desc) => {
                info!(
                    "{} owner {} in {} meta chain",
                    check_type,
                    owner,
                    MetaMinerTarget::default()
                );
                Some(desc)
            }
            Err(_e) => {
                error!(
                    "{} owner {} not in {} meta chain!",
                    check_type,
                    owner,
                    MetaMinerTarget::default()
                );
                None
            }
        }
    } else {
        warn!("device has no owner, are you sure?");
        None
    }
}

#[async_trait]
impl CheckCore for CyfsDescCheck {
    async fn check(&self, check_type: CheckType) -> bool {
        // 没激活的设备通不过base检测
        let device_file = cyfs_util::get_service_config_dir("desc").join("device.desc");
        match cyfs_base::Device::decode_from_file(&device_file, &mut vec![]) {
            Ok((device, _)) => {
                let device_id = device.desc().calculate_id();
                info!("{} device id {}", check_type, &device_id);
                // 检查Device是否有owner, owner是否在链上
                let meta_client = MetaClient::new_target(MetaMinerTarget::default());
                if let Some(owner) = check_owner(check_type, &meta_client, &device).await {
                    if let SavedMetaObject::People(p) = owner {
                        // 检查owner的ood_list，在OOD上还检查DeviceId是否正确
                        check_ood_list(check_type, &p, &device_id, &meta_client).await;
                    } else {
                        warn!("{} owner {} is not people, sure?", check_type, owner.id())
                    }
                } else {
                    warn!("{} owner check failed.", check_type);
                }
            }
            Err(e) => {
                error!(
                    "{} Decode Device {} err {}",
                    check_type,
                    device_file.display(),
                    e
                );
                return false;
            }
        }

        true
    }

    fn name(&self) -> &str {
        "Desc Check"
    }
}

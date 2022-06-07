use base::LOCAL_DEVICE_MANAGER;
use cyfs_base::*;
use cyfs_base_meta::*;
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};

pub struct DescUploader {
    meta_client: MetaClient,
}

impl DescUploader {
    pub fn new() -> DescUploader {
        let meta_client = MetaClient::new_target(MetaMinerTarget::default());

        DescUploader { meta_client }
    }

    pub async fn upload(&self) -> BuckyResult<()> {
        let device_info = match LOCAL_DEVICE_MANAGER.load("device") {
            Ok(v) => v,
            Err(e) => {
                return Err(e);
            }
        };

        match self
            .meta_client
            .create_desc(
                &StandardObject::Device(device_info.device.clone()),
                &SavedMetaObject::Device(device_info.device.clone()),
                0,
                0,
                0, &device_info.private_key.as_ref().unwrap(),
            )
            .await
        {
            Ok(hash) => {
                info!(
                    "create desc on meta success! txid={}, device={}",
                    hash,
                    &device_info.device.desc().device_id()
                );
            }
            Err(e) => {
                let msg = format!(
                    "create desc on meta error! peerid={}, err={}",
                    &device_info.device.desc().device_id(),
                    e
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        Ok(())
    }
}

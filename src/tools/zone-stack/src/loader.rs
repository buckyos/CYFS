use cyfs_base::*;
use cyfs_stack_loader::{DeviceInfo, LOCAL_DEVICE_MANAGER, CyfsServiceLoader};
use zone_simulator::{TestLoader, TestStack};

use crate::profile::*;

pub struct Loader {}

impl Loader {
    pub async fn load() -> BuckyResult<()> {
        // 首先加载配置
        let profile = Profile::load()?;

        let device_info = if let Ok(device_info) = LOCAL_DEVICE_MANAGER.load("device") {
            info!(
                "will use {}/device.desc & deivce.sec",
                LOCAL_DEVICE_MANAGER.get_root().display()
            );

            CyfsServiceLoader::prepare_env().await.unwrap();

            device_info
        } else {
            if let Some(zone) = &profile.data.zone {
                Self::load_device_info_from_profile(zone).await?
            } else {
                let msg = format!("zone fileds not found in config.toml!");
                error!("{}", msg);
                return  Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }           
        };

        let device_id = device_info.device.desc().device_id();
        info!("current stack device_id={}", device_id);

        let stack = TestStack::new(device_info);
        stack
            .init(
                profile.data.ws,
                profile.data.bdt_port,
                profile.data.service_port,
            )
            .await;

        info!("init stack success! device_id={}", device_id);

        Ok(())
    }

    async fn load_device_info_from_profile(zone: &ZoneData) -> BuckyResult<DeviceInfo> {
        info!("will use zone data: {:?}", zone);

        // let name = format!("zone-satck-{}", zone.zone_index);

        // 必须加载两个zone的所有设备，添加到协议栈的known_devices，否则因为没上链会导致查找失败
        let (user1, user2) = TestLoader::load_users(&zone.mnemonic, true, false).await;

        let user;
        if zone.zone_index == 0 {
            user = user1;
        } else if zone.zone_index == 1 {
            user = user2;
        } else {
            unreachable!();
        }

        let info = if zone.device_type == "ood" {
            user.ood
        } else {
            if zone.device_index == 0 {
                user.device1
            } else if zone.device_index == 1 {
                user.device2
            } else {
                unreachable!();
            }
        };

        Ok(info)
    }
}

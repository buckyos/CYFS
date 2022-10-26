use cyfs_base::*;
use cyfs_stack_loader::{DeviceInfo, LOCAL_DEVICE_MANAGER, CyfsServiceLoader};
use zone_simulator::{TestLoader, TestStack};

pub async fn load(is_sim: bool) -> BuckyResult<()> {

    if is_sim { 
        zone_simulator::TEST_PROFILE.load();

        zone_simulator::TestLoader::load_default().await;
    } else {
        let device_info = if let Ok(device_info) = LOCAL_DEVICE_MANAGER.load("device") {
            info!(
                "will use {}/device.desc & deivce.sec",
                LOCAL_DEVICE_MANAGER.get_root().display()
            );

            CyfsServiceLoader::prepare_env().await.unwrap();

            device_info
        } else {
            let msg = format!("{}/device.desc/sec not found", LOCAL_DEVICE_MANAGER.get_root().display());
            error!("{}", msg);
            return  Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        };

        let device_id = device_info.device.desc().device_id();
        info!("current stack device_id={}", device_id);

        let ws = true;
        let bdt_port = 9527;
        let server_port = 9600;
        let stack = TestStack::new(device_info);
        stack
            .init(
                ws,
                bdt_port,
                server_port,
            )
            .await;

        info!("init stack success! device_id={}", device_id);
    }

    info!("init all zones success!");
    Ok(())
}

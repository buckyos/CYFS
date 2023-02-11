use crate::config::DeviceConfigManager;
use crate::init_system_config;

async fn test() {
    init_system_config().await.unwrap();

    let config_manager = DeviceConfigManager::new();
    config_manager.init().unwrap();

    config_manager.fetch_config().await.unwrap();
    config_manager.fetch_config().await.unwrap();
}

#[test]
fn main() {
    cyfs_util::init_log("test-config-repo", Some("debug"));
    async_std::task::block_on(async move {
        test().await;
    });
}
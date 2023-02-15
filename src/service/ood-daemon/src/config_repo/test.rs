use crate::config::DEVICE_CONFIG_MANAGER;
use crate::init_system_config;

async fn test() {
    init_system_config().await.unwrap();
    
    DEVICE_CONFIG_MANAGER.init().unwrap();

    DEVICE_CONFIG_MANAGER.fetch_config().await.unwrap();
    DEVICE_CONFIG_MANAGER.fetch_config().await.unwrap();
}

#[test]
fn main() {
    cyfs_util::init_log("test-config-repo", Some("debug"));
    async_std::task::block_on(async move {
        test().await;
    });
}
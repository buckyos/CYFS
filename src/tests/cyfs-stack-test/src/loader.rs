use zone_simulator::CyfsStackInsConfig;

pub async fn load() {
    zone_simulator::TEST_PROFILE.load();

    let stack_config = CyfsStackInsConfig::default();
    zone_simulator::TestLoader::load_default(&stack_config).await;
    
    info!("init all zones success!");
}

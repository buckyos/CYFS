pub async fn load() {
    zone_simulator::TEST_PROFILE.load();

    zone_simulator::TestLoader::load_default().await;
    
    info!("init all zones success!");
}

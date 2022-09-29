use zone_simulator::*;

mod non_beta;
mod router_handler_beta;
mod root_state_beta;

pub async fn test() {
    // non/router-handler/root-state/rmeta 目前rust的用例至少要覆盖这几大模块
    // [source-zone, source-dec-id, target-zone, target-dec-id]

    root_state_beta::test().await;
    non_beta::test().await;

    router_handler_beta::test().await;

    info!("test all case success!");
}
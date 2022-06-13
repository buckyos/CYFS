use zone_simulator::*;

mod mnemonic;
mod router_handlers;
mod trans;
mod util;
//mod acl;
mod admin;
mod app_manager;
mod codec;
mod crypto;
mod events;
mod ndn;
mod non;
mod non_file;
mod non_handlers;
mod perf;
mod root_state;
mod sync;
mod test_drive;
mod test_obj_searcher;
mod zone;

pub async fn test_restart() {
    let stack = TestLoader::get_stack(DeviceIndex::User1OOD);
    stack.restart_interface().await.unwrap();

    async_std::task::sleep(std::time::Duration::from_secs(3)).await;
}

pub async fn test() {
    codec::test().await;
    test_restart().await;

    test_obj_searcher::test().await;

    // test_drive::test().await;

    events::test().await;
    // crypto::test().await;
    zone::test().await;

    // perf::test().await;

    util::test().await;
    root_state::test().await;

    ndn::test().await;

    non_handlers::test().await;
    non::test().await;
    //non_file::test().await;

    trans::test().await;

    router_handlers::test().await;

    //mnemonic::test().await;
    app_manager::test().await;

    admin::test().await;
    sync::test().await;

    info!("test all case success!");
}
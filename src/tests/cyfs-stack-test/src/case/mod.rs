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
mod noc;
mod non_file;
mod non_handlers;
mod perf;
mod root_state;
mod sync;
mod test_drive;
mod test_obj_searcher;
mod zone;
mod role;
mod storage;
mod meta;
mod call;
mod mime;
mod object_meta_access;
mod shared_stack;
mod context;
mod backup;
mod acl_handler;

pub async fn test_restart() {
    let stack = TestLoader::get_stack(DeviceIndex::User1OOD);
    stack.restart_interface().await.unwrap();

    async_std::task::sleep(std::time::Duration::from_secs(3)).await;
}

pub async fn test() {
    util::test().await;
    return;
    
    acl_handler::test().await;
    backup::test().await;

    async_std::task::spawn(async move {
        shared_stack::test().await;
    });

    test_restart().await;

    // role::test().await;
    // crypto::test().await;

    noc::test().await;
    non::test().await;
    codec::test().await;
    meta::test().await;
    util::test().await;
    context::test().await;
    root_state::test().await;
    mime::test().await;
    ndn::test().await;
    call::test().await;
    object_meta_access::test().await;

    test_obj_searcher::test().await;
    events::test().await;
    zone::test().await;
    // mnemonic::test().await;
    app_manager::test().await;
    trans::test().await;

    admin::test().await;
    sync::test().await;

    async_std::task::sleep(std::time::Duration::from_secs(60 * 30)).await;

    // test_drive::test().await;

    // non_handlers::test().await;
    // non_file::test().await;

    // router_handlers::test().await;

    info!("test all case success!");
}
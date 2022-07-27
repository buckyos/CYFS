use cyfs_base::*;
use cyfs_core::*;
use cyfs_util::Perf;
use cyfs_lib::*;
use cyfs_perf_client::{PerfClient, PerfIsolate, PerfServerConfig};
use zone_simulator::*;

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!("generage test perf  dec_id={}, people={}", dec_id, owner_id);

    dec_id
}

pub async fn test() {
    let dec_id = new_dec("test-perf");
    //let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
    let stack = SharedCyfsStack::open_runtime(Some(dec_id)).await.unwrap();
    stack.wait_online(None).await.unwrap();
    let perf = PerfClient::new(
        "test-perf".to_owned(),
        "1.0.0".to_owned(),
        60,
        Some(dec_id),
        PerfServerConfig::Default,
        stack,
    );
    perf.start().await;

    let isolate = perf.new_isolate("main");

    //test_request(isolate.clone()).await;
    // test_acc(isolate.clone()).await;
    // test_action(isolate.clone()).await;
    // test_record(isolate.clone()).await;

    // async_std::task::sleep(std::time::Duration::from_secs(1000)).await;
}

async fn test_request(perf: PerfIsolate) {
    async_std::task::spawn(async move {
        loop {
            perf.begin_request("connect", "address");

            async_std::task::sleep(std::time::Duration::from_secs(5)).await;

            let _ = perf.end_request("connect", "address", BuckyErrorCode::Ok, Some(100));
        }
    });
}

// async fn test_acc(perf: PerfIsolate) {
//     async_std::task::spawn(async move {
//         loop {
//             async_std::task::sleep(std::time::Duration::from_secs(1)).await;

//             let _ = perf.acc("total", Ok(100));
//         }
//     });
// }

// async fn test_action(perf: PerfIsolate) {
//     async_std::task::spawn(async move {
//         loop {
//             async_std::task::sleep(std::time::Duration::from_secs(10)).await;

//             let _ = perf.action("total", Ok(("drive".into(), "dsg".into())));
//         }
//     });
// }

// async fn test_record(perf: PerfIsolate) {
//     async_std::task::spawn(async move {
//         loop {
//             async_std::task::sleep(std::time::Duration::from_secs(10)).await;

//             let _ = perf.record("total", 100, Some(100));
//         }
//     });
// }
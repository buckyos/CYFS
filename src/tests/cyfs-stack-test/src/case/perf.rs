use std::time::Duration;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_perf_client::{PerfClient, PerfConfig, PerfIsolateInstance, PerfServerConfig};
use zone_simulator::*;

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!("generage test perf  dec_id={}, people={}", dec_id, owner_id);

    dec_id
}

pub async fn test() {
    let dec_id = new_dec("test-perf");
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);
    stack.online().await;
    let device = stack.local_device();
    let owner = device.desc().owner().as_ref().unwrap().clone();
    let device_id = device.desc().device_id();

    let perf = PerfClient::new(
        "test-perf".to_owned(),
        "1.0.0".to_owned(),
        Some(dec_id),
        PerfConfig {
            reporter: PerfServerConfig::Default,
            save_to_file: false,
            report_interval: Duration::from_secs(60*10),
        },
        stack.uni_stack().clone(),
        device_id,
        owner
    );

    let ret = perf.start().await;
    assert!(ret.is_ok());

    let isolate = perf.get_isolate("main");
    test_flush(perf.clone()).await;
    test_request(isolate.clone()).await;
    test_acc(isolate.clone()).await;

    // async_std::task::sleep(std::time::Duration::from_secs(1000)).await;
}

async fn test_request(perf: PerfIsolateInstance) {
    async_std::task::spawn(async move {
        loop {
            perf.begin_request("connect", "address");

            async_std::task::sleep(std::time::Duration::from_secs(1)).await;

            perf.end_request("connect", "address", BuckyErrorCode::Ok, None);
        }
    });
}

async fn test_acc(perf: PerfIsolateInstance) {
    async_std::task::spawn(async move {
        loop {
            async_std::task::sleep(std::time::Duration::from_secs(1)).await;

            perf.acc("total", BuckyErrorCode::Ok, Some(100));
        }
    });
}

async fn test_flush(perf_client: PerfClient) {
    async_std::task::spawn(async move {
        loop {
            async_std::task::sleep(std::time::Duration::from_secs(80)).await;

            perf_client.flush().await.unwrap();
        }
    });
}
use cyfs_base::*;
use zone_simulator::*;

mod mnemonic;
mod router_handlers;
mod trans;
mod util;
//mod acl;
mod app_manager;
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
mod admin;


const BDT_ENDPOINT_CONFIG: &str = r#"
[[stack.bdt.endpoint]]
optional = true
host = "${ip_v4}"
port = ${bdt_port}
protocol = "tcp"

[[stack.bdt.endpoint]]
optional = true
host = "${ip_v4}"
port = ${bdt_port}
protocol = "udp"

[[stack.bdt.endpoint]]
optional = true
host = "::"
port = ${bdt_port}
protocol = "tcp"

[[stack.bdt.endpoint]]
optional = true
host = "::"
port = ${bdt_port}
protocol = "udp"
"#;

pub async fn test_restart() {
    let stack = TestLoader::get_stack(DeviceIndex::User1OOD);
    stack.restart_interface().await.unwrap();
}

use cyfs_lib::*;

pub async fn test() {
    let ret = BDT_ENDPOINT_CONFIG
        .replace("${ip_v4}", "127.0.0.1")
        .replace("${bdt_port}", "1001");
    let _endpoints = cyfs_stack_loader::CyfsServiceLoader::load_endpoints(&ret).unwrap();

    let task = TransControlTaskOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: None,
            level: Default::default(),
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        task_id: "".to_string(),
        action: TransTaskControlAction::Start,
    };
    let value = task.encode_string();
    info!("json value: {}", value);
    TransControlTaskOutputRequest::decode_string(&value).unwrap();

    // test_obj_searcher::test().await;

    // test_restart().await;
    // test_drive::test().await;

    // events::test().await;
    // crypto::test().await;
    // zone::test().await;

    // perf::test().await;

    // util::test().await;
    // root_state::test().await;

    // ndn::test().await;

    //non_handlers::test().await;
    //non::test().await;
    //non_file::test().await;

    // trans::test().await;

    //router_handlers::test().await;
    //util::test().await;

    //mnemonic::test().await;
    // app_manager::test().await;

    admin::test().await;
    sync::test().await;

    info!("test all case success!");
}

use cyfs_base::*;
use cyfs_lib::*;

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
}

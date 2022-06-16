use cyfs_base::*;
use cyfs_meta_lib::MetaMinerTarget;
use cyfs_noc::*;

use async_std::net::SocketAddr;


#[derive(Debug, Clone)]
pub struct CyfsStackParams {
    // 内部使用的noc存储类型
    pub noc_type: NamedObjectStorageType,

    // 隔离配置文件和数据库使用的isolate
    pub isolate: Option<String>,

    // bdt协议栈监听的vport列表
    pub bdt_listeners: Vec<u16>,

    // tcp协议监听的地址列表
    pub tcp_listeners: Vec<SocketAddr>,

    // ws event服务地址
    pub ws_listener: Option<SocketAddr>,

    // 是否开启sync服务
    pub sync_service: bool,

    // 是否开启shared_object_stack服务，默认为true
    pub shared_stack: bool,

    // meta miner的类型
    pub meta: MetaMinerTarget,

    // if enable the front module
    pub front: bool,
}

impl CyfsStackParams {
    pub fn new_empty() -> Self {
        // 初始化两个标准地址
        Self {
            noc_type: NamedObjectStorageType::default(),
            isolate: None,
            tcp_listeners: Vec::new(),
            bdt_listeners: Vec::new(),
            ws_listener: None,
            sync_service: true,
            shared_stack: true,
            meta: MetaMinerTarget::default(),
            front: false,
        }
    }

    pub fn new_default() -> Self {
        // 初始化两个标准地址
        let bdt_listeners = vec![cyfs_base::NON_STACK_BDT_VPORT];
        let tcp_listener: SocketAddr = format!("127.0.0.1:{}", cyfs_base::NON_STACK_HTTP_PORT)
            .parse()
            .unwrap();
        let tcp_listeners = vec![tcp_listener];

        // 默认开启ws服务
        let ws_listener: SocketAddr = format!("127.0.0.1:{}", cyfs_base::NON_STACK_WS_PORT)
            .parse()
            .unwrap();

        Self {
            noc_type: NamedObjectStorageType::default(),
            isolate: None,
            tcp_listeners,
            bdt_listeners,
            ws_listener: Some(ws_listener),
            sync_service: true,
            shared_stack: true,
            meta: MetaMinerTarget::default(),
            front: false,
        }
    }
}

pub struct BdtStackParams {
    pub device: Device,
    pub tcp_port_mapping: Vec<(Endpoint, u16)>,
    pub secret: PrivateKey,
    pub known_sn: Vec<Device>,
    pub known_device: Vec<Device>,
    pub known_passive_pn: Vec<Device>,
    pub udp_sn_only: Option<bool>,
}

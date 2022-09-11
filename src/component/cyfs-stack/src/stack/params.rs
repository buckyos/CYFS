use cyfs_base::*;
use cyfs_meta_lib::MetaMinerTarget;

use async_std::net::SocketAddr;

#[derive(Debug, Clone)]
pub struct CyfsStackConfigParams {
    // 隔离配置文件和数据库使用的isolate
    pub isolate: Option<String>,

    // 是否开启sync服务
    pub sync_service: bool,

    // 是否开启shared_object_stack服务，默认为true
    pub shared_stack: bool,
}

impl Default for CyfsStackConfigParams {
    fn default() -> Self {
        Self {
            isolate: None,
            sync_service: true,
            shared_stack: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CyfsStackFrontParams {
    // if enable the front module
    pub enable: bool,
}

impl Default for CyfsStackFrontParams {
    fn default() -> Self {
        Self { enable: true }
    }
}

#[derive(Debug, Clone)]
pub struct CyfsStackMetaParams {
    // meta miner's type
    pub target: MetaMinerTarget,
}

impl Default for CyfsStackMetaParams {
    fn default() -> Self {
        Self {
            target: MetaMinerTarget::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CyfsStackNOCParams {}

impl Default for CyfsStackNOCParams {
    fn default() -> Self {
        Self {}
    }
}

#[derive(Debug, Clone)]
pub struct CyfsStackInterfaceParams {
    // bdt协议栈监听的vport列表
    pub bdt_listeners: Vec<u16>,

    // tcp协议监听的地址列表
    pub tcp_listeners: Vec<SocketAddr>,

    // ws event服务地址
    pub ws_listener: Option<SocketAddr>,
}

impl Default for CyfsStackInterfaceParams {
    fn default() -> Self {
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
            tcp_listeners,
            bdt_listeners,
            ws_listener: Some(ws_listener),
        }
    }
}

impl CyfsStackInterfaceParams {
    pub fn new_empty() -> Self {
        Self {
            tcp_listeners: Vec::new(),
            bdt_listeners: Vec::new(),
            ws_listener: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CyfsStackParams {
    pub config: CyfsStackConfigParams,

    // noc module
    pub noc: CyfsStackNOCParams,

    // interface module
    pub interface: CyfsStackInterfaceParams,

    // meta module config
    pub meta: CyfsStackMetaParams,

    // front module config
    pub front: CyfsStackFrontParams,
}

impl CyfsStackParams {
    pub fn new_empty() -> Self {
        // 初始化两个标准地址
        Self {
            config: CyfsStackConfigParams::default(),
            noc: CyfsStackNOCParams::default(),
            interface: CyfsStackInterfaceParams::new_empty(),
            meta: CyfsStackMetaParams::default(),
            front: CyfsStackFrontParams::default(),
        }
    }

    pub fn new_default() -> Self {
        Self {
            config: CyfsStackConfigParams::default(),
            noc: CyfsStackNOCParams::default(),
            interface: CyfsStackInterfaceParams::default(),
            meta: CyfsStackMetaParams::default(),
            front: CyfsStackFrontParams::default(),
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

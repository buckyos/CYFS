// service的端口分配 [1300-1400]
pub const CHUNK_MANAGER_PORT: u16 = 1310;

pub const FILE_MANAGER_PORT: u16 = 1312;

pub const ACC_SERVICE_PORT: u16 = 1313;

pub const GATEWAY_CONTROL_PORT: u16 = 1314;

// non-stack本地提供的默认object http服务端口
pub const NON_STACK_HTTP_PORT: u16 = 1318;

// non-stack的本地web-socket服务端口
// TODO 目前tide+async_h1还不支持websocket协议，所以只能使用独立端口
pub const NON_STACK_WS_PORT: u16 = 1319;

// ood-daemon的控制接口
pub const OOD_DAEMON_CONTROL_PORT: u16 = 1320;

// cyfs-runtime的device控制接口，和ood-daemon控制协议一致
pub const CYFS_RUNTIME_DAEMON_CONTROL_PORT: u16 = 1321;

// ood-installer的device控制接口，和ood-daemon控制协议一致
pub const OOD_INSTALLER_CONTROL_PORT: u16 = 1325;

// non-stack本地提供的默认object http服务端口
pub const CYFS_RUNTIME_NON_STACK_HTTP_PORT: u16 = 1322;

// non-stack的本地web-socket服务端口
// TODO 目前tide+async_h1还不支持websocket协议，所以只能使用独立端口
pub const CYFS_RUNTIME_NON_STACK_WS_PORT: u16 = 1323;

// ood-daemon's local status service port
pub const OOD_DAEMON_LOCAL_STATUS_PORT: u16 = 1330;

// bdt协议栈的默认绑定端口
pub const OOD_BDT_STACK_PORT: u16 = 8050;
pub const CYFS_RUNTIME_BDT_STACK_PORT: u16 = 8051;

// non-stack提供对外服务的bdt协议栈虚端口
pub const NON_STACK_BDT_VPORT: u16 = 84;

// non-stack提供对外服务的sync协议栈虚端口
pub const NON_STACK_SYNC_BDT_VPORT: u16 = 85;

// app的端口分配 [1400-1500]
pub const PROXY_MINER_SOCKS5_PORT: u16 = 1421;

pub const IP_RELAY_MINER_PORT: u16 = 1422;

pub const CYFS_META_MINER_PORT: u16 = 1423;

pub const CACHE_MINER_PORT: u16 = 1424;

pub const DNS_PROXY_MINER_PORT: u16 = 1425;

pub const ALWAYS_RUN_MINER_PORT: u16 = 1426;

pub const DSG_CHAIN_MINER_PORT: u16 = 1427;

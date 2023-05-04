use std::str::FromStr;

pub const CYFS_CURRENT_API_EDITION: u32 = 1;

// pub const CYFS_OBJECT_MIME_STRING: &str = "application/cyfs-object";

// 在ios+rn环境下，使用非标准MINE传输二进制会导致fetch端收到错误的数据，这里先改成标准的二进制MINE
pub const CYFS_OBJECT_MIME_STRING: &str = "application/octet-stream";

lazy_static::lazy_static! {
    pub static ref CYFS_OBJECT_MIME: http_types::Mime =  http_types::Mime::from_str(CYFS_OBJECT_MIME_STRING).unwrap();
}

////// ws的cmd定义
// CMD=0 is response，(0, (u16::MAX-32)) is user request, [u16::MAX - 32, u16::MAX] for reserved

// events
pub const ROUTER_WS_EVENT_CMD_ADD: u16 = 1;
pub const ROUTER_WS_EVENT_CMD_REMOVE: u16 = 2;
pub const ROUTER_WS_EVENT_CMD_EVENT: u16 = 3;

// router_handlers
pub const ROUTER_WS_HANDLER_CMD_ADD: u16 = 11;
pub const ROUTER_WS_HANDLER_CMD_REMOVE: u16 = 12;
pub const ROUTER_WS_HANDLER_CMD_EVENT: u16 = 13;

// WS based http request
pub const HTTP_CMD_REQUEST: u16 = 21;

// [u16::MAX - 32, u16::MAX] for reserved
pub const WS_CMD_RESERVED_BEGIN: u16 = u16::MAX - 32;

// An error occurred while processing the request
pub const WS_CMD_ERROR: u16 = u16::MAX;

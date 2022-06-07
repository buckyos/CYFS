use cyfs_base::BuckyError;
use std::str::FromStr;

// pub const CYFS_OBJECT_MIME_STRING: &str = "application/cyfs-object";

// 在ios+rn环境下，使用非标准MINE传输二进制会导致fetch端收到错误的数据，这里先改成标准的二进制MINE
pub const CYFS_OBJECT_MIME_STRING: &str = "application/octet-stream";

lazy_static::lazy_static! {
    pub static ref CYFS_OBJECT_MIME: http_types::Mime =  http_types::Mime::from_str(CYFS_OBJECT_MIME_STRING).unwrap();
}

#[derive(Clone, Eq, Debug, PartialEq)]
pub enum NONProtocol {
    Native,
    Meta,
    Sync,
    HttpBdt,
    HttpLocal,
    HttpLocalAuth,
    DatagramBdt,
    // bdt层的chunk数据传输
    DataBdt,
}

impl NONProtocol {
    pub fn is_local(&self) -> bool {
        match *self {
            Self::Native | Self::HttpLocal | Self::HttpLocalAuth => true,
            Self::HttpBdt | Self::DatagramBdt | Self::DataBdt => false,
            Self::Meta | Self::Sync => false,
        }
    }

    pub fn is_remote(&self) -> bool {
        !self.is_local()
    }

    pub fn is_require_acl(&self) -> bool {
        match *self {
            Self::HttpBdt | Self::DatagramBdt | Self::DataBdt => true,
            Self::Native | Self::HttpLocal | Self::Meta | Self::Sync | Self::HttpLocalAuth => false,
        }
    }
}

impl ToString for NONProtocol {
    fn to_string(&self) -> String {
        (match *self {
            Self::Native => "native",
            Self::Meta => "meta",
            Self::Sync => "sync",
            Self::HttpBdt => "http-bdt",
            Self::HttpLocal => "http-local",
            Self::HttpLocalAuth => "http-local-auth",
            Self::DatagramBdt => "datagram-bdt",
            Self::DataBdt => "data-bdt",
        })
        .to_owned()
    }
}

impl FromStr for NONProtocol {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "native" => Self::Native,
            "meta" => Self::Meta,
            "sync" => Self::Sync,
            "http-bdt" => Self::HttpBdt,
            "http-local" => Self::HttpLocal,
            "http-local-auth" => Self::HttpLocalAuth,
            "datagram-bdt" => Self::DatagramBdt,
            "data-bdt" => Self::DataBdt,
            v @ _ => {
                let msg = format!("unknown non input protocol: {}", v);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        Ok(ret)
    }
}


////// ws的cmd定义
// CMD=0表示是response，大于0表示request

// events
pub const ROUTER_WS_EVENT_CMD_ADD: u16 = 1;
pub const ROUTER_WS_EVENT_CMD_REMOVE: u16 = 2;
pub const ROUTER_WS_EVENT_CMD_EVENT: u16 = 3;

// router_handlers
pub const ROUTER_WS_HANDLER_CMD_ADD: u16 = 11;
pub const ROUTER_WS_HANDLER_CMD_REMOVE: u16 = 12;
pub const ROUTER_WS_HANDLER_CMD_EVENT: u16 = 13;

// 基于ws的http request
pub const HTTP_CMD_REQUEST: u16 = 21;
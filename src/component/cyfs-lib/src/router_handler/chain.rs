use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};


use std::fmt;
use std::str::FromStr;


#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
pub enum RouterHandlerChain {
    PreNOC,
    PostNOC,

    PreRouter,
    PostRouter,

    PreForward,
    PostForward,

    PreCrypto,
    PostCrypto,

    Handler,

    Acl, 

    NDN,
}

impl fmt::Display for RouterHandlerChain {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = match &*self {
            Self::PreNOC => "pre_noc",
            Self::PostNOC => "post_noc",

            Self::PreRouter => "pre_router",
            Self::PostRouter => "post_router",

            Self::PreForward => "pre_forward",
            Self::PostForward => "post_forward",

            Self::PreCrypto => "pre_crypto",
            Self::PostCrypto => "post_crypto",

            Self::Handler => "handler",

            Self::Acl => "acl",

            Self::NDN => "ndn"
        };

        fmt::Display::fmt(s, f)
    }
}

impl FromStr for RouterHandlerChain {
    type Err = BuckyError;
    fn from_str(s: &str) -> BuckyResult<Self> {
        let ret = match s {
            "pre_noc" => Self::PreNOC,
            "post_noc" => Self::PostNOC,

            "pre_router" => Self::PreRouter,
            "post_router" => Self::PostRouter,

            "pre_forward" => Self::PreForward,
            "post_forward" => Self::PostForward,

            "pre_crypto" => Self::PreCrypto,
            "post_crypto" => Self::PostCrypto,

            "handler" => Self::Handler,
            
            "acl" => Self::Acl, 

            "ndn" => Self::NDN, 
            
            v @ _ => {
                let msg = format!("unknown router chain: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        Ok(ret)
    }
}
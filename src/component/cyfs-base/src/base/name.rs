use crate::codec as cyfs_base;
use crate::*;

use std::collections::HashMap;
use std::net::IpAddr;

// name的最大长度(字符个数)，需要小于base58编码后的object_id长度
pub const CYFS_NAME_MAX_LENGTH: usize = 40;

//name信息和obj-desc信息的一个核心不同就是不具备 自校验 性
//使用name info的client必须通过可信的渠道（比如MetaChain)来确认了name当下的owner后，才能对其NameInfo进行检验
#[derive(Clone, Copy, Eq, PartialEq, Debug, RawEncode, RawDecode)]
#[repr(u8)]
pub enum NameState {
    Normal = 0,
    Lock = 1,
    Auction = 2,            //正常拍卖
    ArrearsAuction = 3,     //欠费拍卖
    ArrearsAuctionWait = 4, //欠费拍卖确认
    ActiveAuction = 5,      //主动拍卖
}

impl From<i32> for NameState {
    fn from(i: i32) -> Self {
        match i {
            0 => NameState::Normal,
            1 => NameState::Lock,
            2 => NameState::Auction,
            3 => NameState::ArrearsAuction,
            4 => NameState::ArrearsAuctionWait,
            5 => NameState::ActiveAuction,
            _ => {
                unimplemented!()
            }
        }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum NameLink {
    ObjectLink(ObjectId),
    OtherNameLink(String),
    IPLink(IpAddr),
}

impl std::fmt::Display for NameLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NameLink::ObjectLink(id) => {
                write!(f, "link to obj {}", id)
            }
            NameLink::OtherNameLink(name) => {
                write!(f, "link to name {}", name)
            }
            NameLink::IPLink(ip) => {
                write!(f, "link to ip {}", ip)
            }
        }
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct NameRecord {
    pub link: NameLink,
    pub user_data: String,
}

//pub struct
//类似DNS 的 TXT记录，现在先简单实现。后续会持续扩展可以在一个Name上合法绑定的信息
#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct NameInfo {
    //子域名记录
    pub sub_records: HashMap<String, NameRecord>,
    //直接记录
    pub record: NameRecord,
    pub owner: Option<ObjectId>,
}
//
// impl RawEncode for NameInfo {
//     fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
//         unimplemented!()
//     }
//
//     fn raw_encode<'a>(&self, _buf: &'a mut [u8]) -> BuckyResult<&'a mut [u8]> {
//         unimplemented!()
//     }
// }
//
// impl <'de> RawDecode<'de> for NameInfo {
//     fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
//         unimplemented!()
//     }
// }

use std::time::SystemTime;
use crate::sn::types::SnServiceReceipt;
use crate::{ReceiptWithSignature};
use cyfs_base::*;

pub type IsRequestReceipt = bool;
#[derive(Copy, Clone, Debug)]
pub enum IsAcceptClient {
    Refuse,
    Accept(IsRequestReceipt)
}

#[derive(Debug, Copy, Clone)]
pub enum ReceiptRequestTime {
    None,
    Wait(SystemTime), // 已经要求提供服务证明，正在等待
    Last(SystemTime), // 上次要求提供服务证明已经提供
}

pub trait SnServiceContractServer {
    // 客户端提交服务清单，检查是否合规，并决定是否继续为其服务
    fn check_receipt(&self, client_peer_desc: &Device, // 客户端desc
                     local_receipt: &SnServiceReceipt, // 本地(服务端)统计的服务清单
                     client_receipt: &Option<ReceiptWithSignature>, // 客户端提供的服务清单
                     last_request_time: &ReceiptRequestTime, // 上次要求服务清单的时间
    ) -> IsAcceptClient; // 是否同意为客户端提供服务

    // 检查指定peer是否获得授权
    fn verify_auth(&self, client_peer_id: &DeviceId) -> IsAcceptClient;
}

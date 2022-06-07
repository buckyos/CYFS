use crate::codec as cyfs_base;
use crate::*;

use std::convert::TryFrom;
use std::ops::{Deref, DerefMut};
//
// #[derive(Clone, Debug)]
// pub struct SNContract {
//     //定价
//     // SN 没有单独的定价，纯粹看时间
//     pub price_per_min : u32,//每分钟有效服务的价格
//     //服务列表
// }
//
// impl RawEncode for SNContract {
//     fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
//         Ok(4)
//     }
//
//     fn raw_encode<'a>(&self, buf: &'a mut [u8], _purpose: &Option<RawEncodePurpose>) -> BuckyResult<&'a mut [u8]> {
//         return self.price_per_min.raw_encode(buf, purpose);
//     }
// }
//
// impl<'de> RawDecode<'de> for SNContract {
//     fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
//         unimplemented!()
//     }
// }

#[derive(Clone, Debug)]
pub struct TrafficContract {
    //定价
    pub price_per_kbytes: u32, //每1KB数据的价格
    //服务列表
    pub avg_ping_ms: Option<u16>,

    pub max_up_bytes: Option<u64>,
    pub max_up_speed: Option<u32>,
    pub min_up_speed: Option<u32>,

    pub max_down_bytes: Option<u64>,
    pub max_down_speed: Option<u32>,
    pub min_down_speed: Option<u32>,
}

impl RawEncode for TrafficContract {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let mut size: usize = 4;
        if self.avg_ping_ms.is_some() {
            size += 2;
        }
        if self.max_up_bytes.is_some() {
            size += 8;
        }
        if self.max_up_speed.is_some() {
            size += 4;
        }
        if self.min_up_speed.is_some() {
            size += 4;
        }
        if self.max_up_speed.is_some() {
            size += 8;
        }
        if self.max_down_speed.is_some() {
            size += 4;
        }
        if self.min_down_speed.is_some() {
            size += 4;
        }

        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf = self
            .price_per_kbytes
            .raw_encode(buf, purpose)
            .map_err(|e| {
                log::error!("TrafficContract::raw_encode/price_per_kbytes error:{}", e);
                e
            })?;
        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for TrafficContract {
    fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        unimplemented!()
    }
}

#[derive(Clone, Debug)]
pub struct ChunkTransContract {
    //TODO:可以根据业务需求不断改进
    //定价
    pub price_per_kbytes: u32, //每1Kb数据的价格
    //服务列表
    pub obj_list: Option<Vec<ObjectId>>, //只传输obj_list中包含的chunk，为None表示无限制
    pub min_speed: Option<u32>,          //单位为 KB/S 最小带框保证
    pub max_speed: Option<u32>,          //单位为 KB/S 最大带宽限制
    pub avg_speed: Option<u32>,          //单位为 KB/S
    pub max_bytes: Option<u64>,          //最大流量限制
}

impl RawEncode for ChunkTransContract {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        unimplemented!()
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!()
    }
}

impl<'de> RawDecode<'de> for ChunkTransContract {
    fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        unimplemented!()
    }
}

// #[derive(Clone, Debug)]
// pub struct DSGContract {
//     //TODO：先不实现
// }
//
// impl RawEncode for DSGContract {
//     fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
//         unimplemented!()
//     }
//
//     fn raw_encode<'a>(&self, _buf: &'a mut [u8]) -> BuckyResult<&'a mut [u8]> {
//         unimplemented!()
//     }
// }
//
// impl<'de> RawDecode<'de> for DSGContract {
//     fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
//         unimplemented!()
//     }
// }

#[derive(Clone, Debug)]
pub enum ServiceContractBody {
    // SN(SNContract),//SN-Miner
    Traffic(TrafficContract),       //Proxy-Miner
    ChunkTrans(ChunkTransContract), //Cache-Miner
}

#[derive(Clone, Debug)]
pub struct ServiceContract {
    buyer: ObjectId,
    seller: ObjectId,
    customer: Option<ObjectId>, //为NULL表示和buyer相同。TODO：是否允许后绑定？
    service_type: u32,
    service_start: u64, //服务生效时间
    service_end: u64,

    coin_id: Option<u8>,          //交易使用的币种,默认为0
    total_price: Option<u64>,     //基于本合约的交易上限
    advance_payment: Option<u64>, //需要buyer充入的最小预付款
    contract_body: ServiceContractBody,
}

impl ServiceContract {
    pub fn new() -> ServiceContract {
        unimplemented!();
    }
}

impl RawEncode for ServiceContract {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        let mut size: usize;

        size = self.buyer.raw_measure(purpose).map_err(|e| {
            log::error!("ServiceContract::raw_measure/buyer error:{}", e);
            e
        })?;

        size += self.seller.raw_measure(purpose).map_err(|e| {
            log::error!("ServiceContract::raw_measure/seller error:{}", e);
            e
        })?;

        if self.customer.is_some() {
            size += self.customer.unwrap().raw_measure(purpose).map_err(|e| {
                log::error!("ServiceContract::raw_measure/customer error:{}", e);
                e
            })?;
        }

        size += self.service_type.raw_measure(purpose).map_err(|e| {
            log::error!("ServiceContract::service_type/customer error:{}", e);
            e
        })?;

        size += self.service_start.raw_measure(purpose).map_err(|e| {
            log::error!("ServiceContract::service_type/service_start error:{}", e);
            e
        })?;

        size += self.service_end.raw_measure(purpose).map_err(|e| {
            log::error!("ServiceContract::service_type/service_end error:{}", e);
            e
        })?;

        if self.coin_id.is_some() {
            size += self.coin_id.unwrap().raw_measure(purpose).map_err(|e| {
                log::error!("ServiceContract::service_type/coin_id error:{}", e);
                e
            })?;
        }

        if self.total_price.is_some() {
            size += self
                .total_price
                .unwrap()
                .raw_measure(purpose)
                .map_err(|e| {
                    log::error!("ServiceContract::service_type/total_price error:{}", e);
                    e
                })?;
        }

        if self.advance_payment.is_some() {
            size += self
                .advance_payment
                .unwrap()
                .raw_measure(purpose)
                .map_err(|e| {
                    log::error!("ServiceContract::service_type/advance_payment error:{}", e);
                    e
                })?;
        }

        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!()
    }
}

impl<'de> RawDecode<'de> for ServiceContract {
    fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        unimplemented!()
    }
}

//-------------------------------------------------------

#[derive(Clone, Debug)]
pub struct SNReceipt {
    pub ping_count: Option<u32>,
    pub called_count: Option<u32>,
    pub success_called_count: Option<u32>, //这个只能在C回填的时候写
}

impl RawEncode for SNReceipt {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        unimplemented!()
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!()
    }
}

impl<'de> RawDecode<'de> for SNReceipt {
    fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        unimplemented!()
    }
}

#[derive(Clone, Debug)]
pub struct TrafficReceipt {
    pub up_bytes: u64,
    pub down_bytes: u64,
    pub total_package: u64,
    pub max_speed: Option<u32>,
    pub min_speed: Option<u32>,
    pub avg_ping_ms: Option<u16>, //单位ms
    pub stream_count: Option<u32>,
    pub failed_stream_count: Option<u32>,
    pub break_stream_count: Option<u32>,
}

impl RawEncode for TrafficReceipt {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        unimplemented!()
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!()
    }
}

impl<'de> RawDecode<'de> for TrafficReceipt {
    fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        unimplemented!()
    }
}

//TODO：使用时可根据需要修改
//C->S 不带crypto_key
//S->C 带crypto_key
//C->B 带crypto_key(通知成功)
//S->B 带cryto_key(要钱）
#[derive(Clone, Debug)]
pub struct ChunkTransReceipt {
    pub chunk_id: ChunkId,
    pub crypto_chunk_id: ChunkId,
    pub valid_length: Option<u64>, //有效传输的数据
    pub max_speed: Option<u32>,
    pub min_speed: Option<u32>, //平均速度可以从Chunk大小和服务持续时间推算出来
    pub crypto_key: Option<u64>, //aes-key的长度是？ 这个字段是S->C的时候填写的
}

impl RawEncode for ChunkTransReceipt {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        unimplemented!()
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!()
    }
}

impl<'de> RawDecode<'de> for ChunkTransReceipt {
    fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        unimplemented!()
    }
}

#[derive(Clone, Debug)]
pub struct DSGReceipt {
    //TODO：先不实现
}

impl RawEncode for DSGReceipt {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        unimplemented!()
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!()
    }
}

impl<'de> RawDecode<'de> for DSGReceipt {
    fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        unimplemented!()
    }
}

#[derive(Clone, Debug)]
pub enum ServiceReceiptBody {
    SN(SNReceipt),
    Traffic(TrafficReceipt),
    ChunkTrans(ChunkTransReceipt),
    DSG(DSGReceipt),
}

//服务证明（存根部分）
//由member of seller创建，发送给customer
//customer如果认可，则进行签名并发回。
//随后member of seller会把一组Receipt发给buyer,要求对方进行支付
#[derive(Clone, Debug)]
pub struct ServiceReceipt {
    customer: ObjectId, // 谁使用服务
    service_type: u32,
    service_start: u64, // 本凭证的开始时间
    service_end: u64,   // 本凭证的结束时间
    receipt_body: ServiceReceiptBody,
}

impl ServiceReceipt {
    fn new() -> ServiceReceipt {
        unimplemented!();
    }
}

impl RawEncode for ServiceReceipt {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        unimplemented!()
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!()
    }
}

impl<'de> RawDecode<'de> for ServiceReceipt {
    fn raw_decode(_buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        unimplemented!()
    }
}

//----------------------------------------
#[derive(Clone, Debug)]
pub enum Service {
    Contract(ServiceContract),
    Receipt(ServiceReceipt),
    //TODO：加入支付Tx的Body(振爷）
}

const CONTRACT_SN_BODY_CODE: u8 = 0_u8;
const CONTRACT_TRAFFIC_BODY_CODE: u8 = 1_u8;
const CONTRACT_CHUNK_TRANS_BODY_CODE: u8 = 2_u8;
const CONTRACT_DSG_BODY_CODE: u8 = 3_u8;
const RECEIPT_SN_BODY_CODE: u8 = 10_u8;
const RECEIPT_TRAFFIC_BODY_CODE: u8 = 11_u8;
const RECEIPT_CHUNK_TRANS_BODY_CODE: u8 = 12_u8;
const RECEIPT_DSG_BODY_CODE: u8 = 13_u8;

#[derive(Clone, Copy, Debug, PartialEq, RawEncode, RawDecode)]
enum SnServiceReceiptVersion {
    Invalid = 0,
    Current = 1,
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Ord, Eq, RawEncode, RawDecode)]
enum SnServiceGrade {
    None = 0,
    Discard = 1,
    Passable = 2,
    Normal = 3,
    Fine = 4,
    Wonderfull = 5,
}

impl SnServiceGrade {
    pub fn is_accept(&self) -> bool {
        *self >= SnServiceGrade::Passable
    }
    pub fn is_refuse(&self) -> bool {
        !self.is_accept()
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
struct ProofOfSNService {
    pub version: SnServiceReceiptVersion,
    pub grade: SnServiceGrade,
    pub rto: u64,
    pub duration: u64,
    pub start_time: u64,
    pub ping_count: u64,
    pub ping_resp_count: u64,
    pub called_count: u64,
    pub call_peer_count: u64,
    pub connect_peer_count: u64,
    pub call_delay: u64,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ProofOfDSG {}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub enum ProofTypeCode {
    DSGStorage,
    DSGStorageCheck,
    DSGMerkleProof,
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ProofOfServiceDescContent<T> {
    pub proof_type: ProofTypeCode,
    pub data: T,
}

impl<T> Deref for ProofOfServiceDescContent<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for ProofOfServiceDescContent<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ProofData {
    data: Vec<u8>,
}

impl<T> DescContent for ProofOfServiceDescContent<T> {
    fn obj_type() -> u16 {
        ObjectTypeCode::ProofOfService.into()
    }

    type OwnerType = Option<ObjectId>;
    type AreaType = SubDescNone;
    type AuthorType = SubDescNone;
    type PublicKeyType = SubDescNone;
}

#[derive(Clone, Debug, RawEncode, RawDecode)]
pub struct ProofOfServiceBodyContent<T> {
    pub data: T,
}

impl<T> BodyContent for ProofOfServiceBodyContent<T> {}

impl<T> Deref for ProofOfServiceBodyContent<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<T> DerefMut for ProofOfServiceBodyContent<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

type ProofOfServiceBodyContentProofData = ProofOfServiceBodyContent<ProofData>;

pub type ProofOfServiceType =
    NamedObjType<ProofOfServiceDescContent<ProofData>, ProofOfServiceBodyContent<ProofData>>;
pub type ProofOfServiceBuilder =
    NamedObjectBuilder<ProofOfServiceDescContent<ProofData>, ProofOfServiceBodyContent<ProofData>>;

pub type ProofOfServiceDesc = NamedObjectDesc<ProofOfServiceDescContent<ProofData>>;
pub type ProofOfServiceId = NamedObjectId<ProofOfServiceType>;
pub type ProofOfService = NamedObjectBase<ProofOfServiceType>;

impl ProofOfServiceDesc {
    pub fn proof_of_service_id(&self) -> ProofOfServiceId {
        ProofOfServiceId::try_from(self.calculate_id()).unwrap()
    }
}

impl NamedObjectBase<ProofOfServiceType> {
    pub fn new(desc_content: ProofOfServiceDescContent<ProofData>) -> ProofOfServiceBuilder {
        let body_content = ProofOfServiceBodyContent {
            data: ProofData { data: Vec::new() },
        };

        ProofOfServiceBuilder::new(desc_content, body_content)
    }
}

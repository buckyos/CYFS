use cyfs_base::*;
use std::convert::From;
use std::convert::Into;

pub enum ChunkGetReqType{
    DataWithMeta,
    DataOrRedirectWithMeta,
    Data,
}

impl From<&u8> for ChunkGetReqType {
    fn from(req_type: &u8) -> Self{
        match req_type {
            0u8=>ChunkGetReqType::DataWithMeta,
            1u8=>ChunkGetReqType::DataOrRedirectWithMeta,
            _=>ChunkGetReqType::Data,
        }
    }
}

impl From<ChunkGetReqType> for u8 {
    fn from(t: ChunkGetReqType) -> u8{
        match t {
            ChunkGetReqType::DataWithMeta => 0u8,
            ChunkGetReqType::DataOrRedirectWithMeta => 1u8,
            ChunkGetReqType::Data=> 2u8,
        }
    }
}

#[derive(RawEncode, RawDecode)]
pub struct ChunkGetReq {
    source_device_id: DeviceId,  
    client_device_id: DeviceId, 
    chunk_id: ChunkId, 
    price: i64,
    req_type: u8,
}

impl ChunkGetReq {

    pub fn source_device_id(&self)->&DeviceId {
        &self.source_device_id
    }

    pub fn client_device_id(&self)->&DeviceId {
        &self.client_device_id
    }

    pub fn chunk_id(&self)->&ChunkId {
        &self.chunk_id
    }

    pub fn price(&self)->&i64 {
        &self.price
    }

    pub fn req_type(&self)->ChunkGetReqType {
        ChunkGetReqType::from(&self.req_type)
    }

    pub fn sign(_client_signer: &PrivateKey, source_device_id:&DeviceId, client_device_id:&DeviceId, chunk_id:&ChunkId, price:&i64, req_type: ChunkGetReqType)->BuckyResult<ChunkGetReq>{
        // TODO
        Ok(ChunkGetReq{
            source_device_id: source_device_id.clone(),
            client_device_id: client_device_id.clone(),
            chunk_id: chunk_id.clone(),
            price: price.clone(),
            req_type: req_type.into(),
        })
    }

    pub fn verify(&self, _client_public_key: &PublicKey)->bool{
        // TODO
        true
    }
}
use cyfs_base::*;
use crate::chunk_get_raw::*;
use crate::chunk_redirect_req::*;

use std::sync::Arc;


pub enum ChunkGetRespStatus{
    Ok = 200,
    Redirect = 302,
    Error = 503,
}

impl From<&u8> for ChunkGetRespStatus {
    fn from(req_type: &u8) -> Self{
        match req_type {
            1u8=>ChunkGetRespStatus::Ok,
            2u8=>ChunkGetRespStatus::Redirect,
            _=>ChunkGetRespStatus::Error,
        }
    }
}

impl From<ChunkGetRespStatus> for u8 {
    fn from(t: ChunkGetRespStatus) -> u8{
        match t {
            ChunkGetRespStatus::Ok => 0u8,
            ChunkGetRespStatus::Redirect => 1u8,
            ChunkGetRespStatus::Error=> 2u8,
        }
    }
}

#[derive(RawEncode, RawDecode)]
#[cyfs(optimize_option)]
pub struct ChunkGetResp {
    raw: Option<ChunkGetRaw>,
    redirect: Option<ChunkRedirectReq>,
    status: u8,
}

impl ChunkGetResp {
    pub fn raw(&self)->&Option<ChunkGetRaw>{
        &self.raw
    }

    pub fn redirect(&self)->&Option<ChunkRedirectReq>{
        &self.redirect
    }

    pub fn status(&self)->ChunkGetRespStatus{
        ChunkGetRespStatus::from(&self.status)
    }

    pub fn new_raw(source_signer: &PrivateKey, source_device_id:&DeviceId,  client_device_id:&DeviceId, chunk_id:&ChunkId, data: Arc<Vec<u8>>)->BuckyResult<ChunkGetResp>{

        let chunk_get_raw = ChunkGetRaw::sign(source_signer, source_device_id, client_device_id, chunk_id, data)?;

        Ok(ChunkGetResp{
            status: ChunkGetRespStatus::Ok.into(),
            raw: Some(chunk_get_raw),
            redirect: None
        })
    }

    pub fn new_redirect(source_signer: &PrivateKey, source_device_id: &DeviceId, miner_device_id: &DeviceId, client_device_id:&DeviceId, chunk_id: &ChunkId, price: &i64)->BuckyResult<ChunkGetResp>{
        let chunk_get_redirect = ChunkRedirectReq::sign(source_signer, source_device_id, miner_device_id, client_device_id, chunk_id, price)?;

        Ok(ChunkGetResp{
            status: ChunkGetRespStatus::Redirect.into(),
            raw: None,
            redirect: Some(chunk_get_redirect)
        })
    }
}

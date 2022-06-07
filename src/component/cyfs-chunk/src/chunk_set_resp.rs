use cyfs_base::*;

pub enum ChunkSetStatus {
    Ok = 200,
    Error = 503,
}

impl From<&u8> for ChunkSetStatus {
    fn from(req_type: &u8) -> Self{
        match req_type {
            0u8=>ChunkSetStatus::Ok,
            _=>ChunkSetStatus::Error,
        }
    }
}

impl From<ChunkSetStatus> for u8 {
    fn from(status: ChunkSetStatus) -> u8{
        match status {
            ChunkSetStatus::Ok=>0,
            ChunkSetStatus::Error=>1,
        }
    }
}

#[derive(RawEncode, RawDecode)]
pub struct ChunkSetResp {
    source_device_id: DeviceId,
    chunk_id: ChunkId,
    status: u8,
    // pub sign: String, 
}

impl ChunkSetResp {
    pub fn source_device_id(&self)->&DeviceId {
        &self.source_device_id
    }

    pub fn chunk_id(&self)->&ChunkId {
        &self.chunk_id
    }

    pub fn status(&self)->ChunkSetStatus {
        ChunkSetStatus::from(&self.status)
    }

    pub fn sign(_source_signer: &PrivateKey, source_device_id:&DeviceId, chunk_id:&ChunkId, status: ChunkSetStatus)->BuckyResult<ChunkSetResp>{
        Ok(Self{
            source_device_id: source_device_id.clone(),
            chunk_id: chunk_id.clone(),
            status: status.into(),
        })
    }

    pub fn verify(&self, _source_public_key: &PublicKey)->bool{
        // TODO

        true
    }
}
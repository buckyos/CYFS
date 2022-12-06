use cyfs_base::*;

use std::sync::Arc;

#[derive(RawEncode, RawDecode)]
pub struct ChunkGetRaw {
    source_device_id: DeviceId,  
    client_device_id: DeviceId, 
    chunk_id: ChunkId, 
    data: Arc<Vec<u8>>,
    // sign: String,
}

impl ChunkGetRaw {

    pub fn source_device_id(&self)->&DeviceId {
        &self.source_device_id
    }

    pub fn client_device_id(&self)->&DeviceId {
        &self.client_device_id
    }

    pub fn chunk_id(&self)->&ChunkId {
        &self.chunk_id
    }

    pub fn data(&self)->&[u8] {
        &self.data.as_slice()
    }

    pub fn sign(_source_signer: &PrivateKey, source_device_id:&DeviceId,  client_device_id:&DeviceId, chunk_id:&ChunkId, data: Arc<Vec<u8>>)->BuckyResult<ChunkGetRaw>{
        // TODO
        Ok(ChunkGetRaw{
            source_device_id: source_device_id.clone(),
            client_device_id: client_device_id.clone(),
            chunk_id: chunk_id.clone(),
            data,
        })
    }

    pub fn verify(&self, _client_public_key: &PublicKey)->bool{
        // TODO
        true
    }
}
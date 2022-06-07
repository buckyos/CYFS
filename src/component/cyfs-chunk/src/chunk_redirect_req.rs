use cyfs_base::*;

#[derive(RawEncode, RawDecode)]
pub struct ChunkRedirectReq {
    source_device_id: DeviceId, 
    miner_device_id: DeviceId, 
    client_device_id: DeviceId,
    chunk_id: ChunkId,
    price: i64,
    // sign: String,
}

impl ChunkRedirectReq {
    pub fn source_device_id(&self)->&DeviceId {
        &self.source_device_id
    }

    pub fn miner_device_id(&self)->&DeviceId {
        &self.miner_device_id
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

    pub fn sign(_source_signer: &PrivateKey, source_device_id: &DeviceId, miner_device_id: &DeviceId, client_device_id:&DeviceId, chunk_id: &ChunkId, price: &i64)->BuckyResult<ChunkRedirectReq>{
        // TODO

        Ok(ChunkRedirectReq{
            source_device_id: source_device_id.clone(),
            miner_device_id: miner_device_id.clone(),
            client_device_id: client_device_id.clone(),
            chunk_id: chunk_id.clone(),
            price: price.clone(),
            // sign: sign.to_owned()
        })
    }

    pub fn verify(&self, _source_public_key: &PublicKey)->bool{
        // TODO
        true
    }
}

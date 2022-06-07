use cyfs_base::*;

pub struct ChunkCacheReq {
    pub miner_device_id: DeviceId,  
    pub source_device_id: DeviceId,  
    pub client_device_id: DeviceId, 
    pub chunk_id: ChunkId, 
    pub price: i64,
    pub sign: String,
}

impl ChunkCacheReq {
    pub fn sign(client_signer: &Signer, miner_device_id:&DeviceId,  source_device_id:&DeviceId, client_device_id:&DeviceId, chunk_id:&ChunkId, price: i64)->BuckyResult<ChunkCacheReq>{
        // let buffer  = [
        //     miner_device_id.to_string().as_bytes(),
        //     source_device_id.to_string().as_bytes(),
        //     client_device_id.to_string().as_bytes(),
        //     chunk_id.to_string().as_bytes(),
        //     &price.to_ne_bytes(),
        // ].concat();

        let mut sign_bytes:[u8; SIGNATRUE_LENGTH] = [0u8; SIGNATRUE_LENGTH];
            
        client_signer.md5_sign(&buffer, &mut sign_bytes).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        let sign = sign_to_string(&sign_bytes);

        Ok(ChunkCacheReq{
            miner_device_id: miner_device_id.clone(),
            source_device_id: source_device_id.clone(),
            client_device_id: client_device_id.clone(),
            chunk_id: chunk_id.clone(),
            price: price.clone(),
            sign: sign
        })
    }

    pub fn verify(&self, client_public_key: &PublicKey)->bool{
        let buffer  = [
            self.miner_device_id.to_string().as_bytes(),
            self.source_device_id.to_string().as_bytes(),
            self.client_device_id.to_string().as_bytes(),
            self.chunk_id.to_string().as_bytes(),
            &self.price.to_ne_bytes(),
        ].concat();

        let ret = sign_from_string(&self.sign);
        if let Err(_e) = ret {
            return false;
        }
        let mut sign_bytes = ret.unwrap();
            
        return client_public_key.verify_md5(&buffer, &mut sign_bytes);
    }
}
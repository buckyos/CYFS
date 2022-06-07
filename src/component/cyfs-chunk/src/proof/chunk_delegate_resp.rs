use serde::{Serialize, Deserialize};
use ffs_base::{Peerid, Chunkid, BuckyResult, BuckyError, PeerSecret, PublicKey, SIGNATRUE_LENGTH};
use bdt2::crypto_tool::{PeerSecretImpl, PublicKeyImpl};
use crate::chunk_base::*;

#[derive(Serialize, Deserialize)]
pub struct ChunkDelegateResp {
    pub source_device_id: Peerid,  
    pub miner_device_id: Peerid, 
    pub chunk_id: Chunkid, 
    pub price: i64,
    pub sign: String,
}

impl ChunkDelegateResp {
    pub fn sign(miner_signer: &PeerSecret, source_device_id:&Peerid,  miner_device_id:&Peerid, chunk_id:&Chunkid, price:&i64)->BuckyResult<ChunkDelegateResp>{
        let buffer  = [
            source_device_id.to_string().as_bytes(),
            miner_device_id.to_string().as_bytes(),
            chunk_id.to_string().as_bytes(),
            &price.to_ne_bytes(),
        ].concat();

        let mut sign_bytes:[u8; SIGNATRUE_LENGTH] = [0u8; SIGNATRUE_LENGTH];
            
        miner_signer.md5_sign(&buffer, &mut sign_bytes).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        let sign = sign_to_string(&sign_bytes);

        Ok(ChunkDelegateResp{
            source_device_id: source_device_id.clone(),
            miner_device_id: miner_device_id.clone(),
            chunk_id: chunk_id.clone(),
            price: price.clone(),
            sign: sign.to_owned()
        })
    }

    pub fn verify(&self, miner_public_key:&PublicKey)->bool{

        let buffer  = [
            self.source_device_id.to_string().as_bytes(),
            self.miner_device_id.to_string().as_bytes(),
            self.chunk_id.to_string().as_bytes(),
            &self.price.to_ne_bytes(),
        ].concat();

        let ret = sign_from_string(&self.sign);
        if let Err(_e) = ret {
            return false;
        }
        let mut sign_bytes = ret.unwrap();
            
        return miner_public_key.verify_md5(&buffer, &mut sign_bytes);
    }
}
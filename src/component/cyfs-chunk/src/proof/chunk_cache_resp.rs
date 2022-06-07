use serde::{Serialize, Deserialize};
use ffs_base::{Peerid, Chunkid, BuckyResult, BuckyError, PeerSecret, PublicKey, SIGNATRUE_LENGTH, AES_KEY_LENGTH};
use bdt2::crypto_tool::{PeerSecretImpl, PublicKeyImpl, BdtAesCryptoSimple};
use crate::chunk_base::*;

#[derive(Serialize, Deserialize)]
pub struct ChunkCacheResp {
    pub source_device_id: Peerid, 
    pub miner_device_id: Peerid, 
    pub client_device_id: Peerid,
    pub chunk_id: Chunkid,
    pub session_id: i64, 
    pub encrypted_data: Vec<u8>,
    pub sign: String,
}

impl ChunkCacheResp {
    pub fn sign(miner_signer: &PeerSecret, aes_key: &[u8; AES_KEY_LENGTH], source_device_id: &Peerid, miner_device_id: &Peerid, client_device_id:&Peerid, chunk_id: &Chunkid, session_id: &i64, data: Vec<u8>)->BuckyResult<ChunkCacheResp>{
        // 初始化加密数据
        let encrypted_data = BdtAesCryptoSimple::encrypt(aes_key, &data).map_err(|e|{
            BuckyError::from(format!("encrypt failed, {:?}", e))
        })?;
        
        let buffer  = [
            source_device_id.to_string().as_bytes(),
            miner_device_id.to_string().as_bytes(),
            client_device_id.to_string().as_bytes(),
            chunk_id.to_string().as_bytes(),
            &session_id.to_ne_bytes(),
            &encrypted_data
        ].concat();

        let mut sign_bytes:[u8; SIGNATRUE_LENGTH] = [0u8; SIGNATRUE_LENGTH];
        miner_signer.md5_sign(&buffer, &mut sign_bytes).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        let sign = sign_to_string(&sign_bytes);

        Ok(ChunkCacheResp{
            source_device_id: source_device_id.clone(),
            miner_device_id: miner_device_id.clone(),
            client_device_id: client_device_id.clone(),
            chunk_id: chunk_id.clone(),
            session_id: session_id.clone(),
            encrypted_data: encrypted_data,
            sign: sign.to_owned()
        })
    }

    pub fn verify(&self, miner_public_key: &PublicKey)->bool{
        let buffer  = [
            self.source_device_id.to_string().as_bytes(),
            self.miner_device_id.to_string().as_bytes(),
            self.client_device_id.to_string().as_bytes(),
            self.chunk_id.to_string().as_bytes(),
            &self.session_id.to_ne_bytes(),
            &self.encrypted_data
        ].concat();

        let ret = sign_from_string(&self.sign);
        if let Err(_e) = ret {
            return false;
        }
        let mut sign_bytes = ret.unwrap();
            
        return miner_public_key.verify_md5(&buffer, &mut sign_bytes);
    }
}
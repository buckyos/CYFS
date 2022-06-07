use serde::{Serialize, Deserialize};
use ffs_base::{Peerid, Chunkid, BuckyResult, BuckyError, PeerSecret, PublicKey, SIGNATRUE_LENGTH, AES_KEY_LENGTH};
use bdt2::crypto_tool::{PeerSecretImpl, PublicKeyImpl, BdtAesCryptoSimple};
use crate::chunk_cache_resp::ChunkCacheResp;
use crate::chunk_base::*;
use log::*;

#[derive(Serialize, Deserialize)]
pub struct ChunkProofResp {
    pub source_device_id: Peerid, 
    pub miner_device_id: Peerid, 
    pub client_device_id: Peerid, 
    pub chunk_id: Chunkid, 
    pub session_id: i64, 
    pub encrypt_aes_key: Vec<u8>,
    pub sign: String,  // sign(source_device_id+miner_device_id+client_device_id+chunk_id+session_id+encrypt_aes_key)
}

impl ChunkProofResp {
    pub fn sign(miner_signer: &PeerSecret,  client_public_key: &PublicKey, source_device_id:&Peerid, miner_device_id:&Peerid, client_device_id:&Peerid, chunk_id:&Chunkid, session_id: &i64, aes_key: &[u8; AES_KEY_LENGTH])->BuckyResult<ChunkProofResp>{

        let mut encrypt_aes_key = vec![0u8;client_public_key.get_type().get_bytes()];
        let _ = client_public_key.encrypt(aes_key, &mut encrypt_aes_key).map_err(|(code,msg)|{
            error!("ChunkProofResp encrypt failed, code:{}, msg:{}", code, msg);
            BuckyError::from(format!("ChunkProofResp encrypt failed, code:{}, msg:{}", code, msg))
        })?;

        let buffer  = [
            source_device_id.to_string().as_bytes(),
            miner_device_id.to_string().as_bytes(),
            client_device_id.to_string().as_bytes(),
            chunk_id.to_string().as_bytes(),
            &session_id.to_ne_bytes(),
            &encrypt_aes_key,
        ].concat();

        let mut sign_bytes:[u8; SIGNATRUE_LENGTH] = [0u8; SIGNATRUE_LENGTH];
            
        miner_signer.md5_sign(&buffer, &mut sign_bytes).map_err(|e|{
            error!("ChunkProofResp sign failed");
            BuckyError::from(e)
        })?;

        let sign = sign_to_string(&sign_bytes);

        Ok(ChunkProofResp{
            source_device_id: source_device_id.clone(),
            miner_device_id: miner_device_id.clone(),
            client_device_id: client_device_id.clone(),
            chunk_id: chunk_id.clone(),
            session_id: session_id.clone(),
            encrypt_aes_key: encrypt_aes_key,
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
            &self.encrypt_aes_key,
        ].concat();

        let ret = sign_from_string(&self.sign);
        if let Err(_e) = ret {
            error!("convert sign from hex string failed");
            return false;
        }
        let mut sign_bytes = ret.unwrap();
            
        return miner_public_key.verify_md5(&buffer, &mut sign_bytes);
    }

    pub fn decode(&self, client_signer: &PeerSecret, chunk_cache_resp:&ChunkCacheResp)->BuckyResult<Vec<u8>>{
        // 解密aes_key    
        let mut aes_key = [0u8; AES_KEY_LENGTH];
        let (_, _) = client_signer.decrypt(&self.encrypt_aes_key, &mut aes_key)?;

        // 使用解密数据
        let _ = (chunk_cache_resp.encrypted_data.len() + 15) / 16 * 16;
        let data = BdtAesCryptoSimple::decrypt(&aes_key, &chunk_cache_resp.encrypted_data).map_err(|e|{
            BuckyError::from(format!("decrypt failed, {:?}", e))
        })?;

        Ok(data)
    }
}
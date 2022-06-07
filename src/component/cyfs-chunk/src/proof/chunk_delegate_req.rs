use serde::{Serialize, Deserialize};
use ffs_base::{Peerid, Chunkid, BuckyResult, BuckyError, PeerSecret, PublicKey, SIGNATRUE_LENGTH, AES_KEY_LENGTH};
use bdt2::crypto_tool::{PeerSecretImpl, PublicKeyImpl, BdtAesCryptoSimple};
use crate::chunk_base::*;

#[derive(Serialize, Deserialize)]
pub struct ChunkDelegateReq {
    pub source_device_id: Peerid,  
    pub miner_device_id: Peerid, 
    pub chunk_id: Chunkid, 
    pub price: i64,
    pub encrypt_ase_key: Vec<u8>,
    pub encrypt_data: Vec<u8>,
    pub sign: String,
}

impl ChunkDelegateReq {
    pub fn sign(source_signer: &PeerSecret, miner_public_key:&PublicKey, source_device_id:&Peerid,  miner_device_id:&Peerid, chunk_id:&Chunkid, price:&i64, data:Vec<u8>)->BuckyResult<ChunkDelegateReq>{
        
        // gen ase public key
        let aes_key = BdtAesCryptoSimple::generate();

        // rsa encrypt ase public key
        let mut encrypt_ase_key = vec![0u8;miner_public_key.get_type().get_bytes()];
        let _ = miner_public_key.encrypt(&aes_key,& mut encrypt_ase_key).map_err(|e|{
            BuckyError::from(format!("encrypt ase key for miner failed, {:?}", e))
        })?;

        // encrypt data
        let encrypt_data = BdtAesCryptoSimple::encrypt(&aes_key, &data).map_err(|e|{
            BuckyError::from(format!("encrypt data for miner failed, {:?}", e))
        })?;
        
        let buffer  = [
            source_device_id.to_string().as_bytes(),
            miner_device_id.to_string().as_bytes(),
            chunk_id.to_string().as_bytes(),
            &price.to_ne_bytes(),
            &encrypt_ase_key,
            &encrypt_data
        ].concat();

        let mut sign_bytes:[u8; SIGNATRUE_LENGTH] = [0u8; SIGNATRUE_LENGTH];
            
        source_signer.md5_sign(&buffer, &mut sign_bytes).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        let sign = sign_to_string(&sign_bytes);

        Ok(ChunkDelegateReq{
            source_device_id: source_device_id.clone(),
            miner_device_id: miner_device_id.clone(),
            chunk_id: chunk_id.clone(),
            price: price.clone(),
            encrypt_ase_key: encrypt_ase_key,
            encrypt_data: encrypt_data,
            sign: sign.to_owned()
        })
    }

    pub fn verify(&self, source_public_key:&PublicKey)->bool{

        let buffer  = [
            self.source_device_id.to_string().as_bytes(),
            self.miner_device_id.to_string().as_bytes(),
            self.chunk_id.to_string().as_bytes(),
            &self.price.to_ne_bytes(),
            &self.encrypt_ase_key,
            &self.encrypt_data
        ].concat();

        let ret = sign_from_string(&self.sign);
        if let Err(_e) = ret {
            return false;
        }
        let mut sign_bytes = ret.unwrap();
            
        return source_public_key.verify_md5(&buffer, &mut sign_bytes);
    }

    pub fn decode(&self, miner_signer: &PeerSecret)->BuckyResult<Vec<u8>>{
        // rsa decrypt ase public key
        let mut aes_key:[u8; AES_KEY_LENGTH] = [0u8; AES_KEY_LENGTH];
        let  (_,_) = miner_signer.decrypt(&self.encrypt_ase_key,& mut aes_key).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        // aes decrypt data
        let data = BdtAesCryptoSimple::decrypt(&aes_key, &self.encrypt_data).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        Ok(data)
    }
}
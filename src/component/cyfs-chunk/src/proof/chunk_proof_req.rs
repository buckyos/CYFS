use serde::{Serialize, Deserialize};
use ffs_base::{Peerid, Chunkid, BuckyResult, BuckyError, PeerSecret, PublicKey, SIGNATRUE_LENGTH};
use bdt2::crypto_tool::{PeerSecretImpl, PublicKeyImpl};
use time::Timespec;
use crate::ChunkRedeemReq;
use crate::chunk_base::*;


#[derive(Serialize, Deserialize)]
pub struct ChunkProofReq {
    pub source_device_id: Peerid, 
    pub miner_device_id: Peerid, 
    pub client_device_id: Peerid, 
    pub chunk_id: Chunkid, 
    pub session_id: i64, 
    pub timestamp_sec: i64,
    pub timestamp_nsec: i32,
    pub proof: String,  // sign(source_device_id+miner_device_id+client_device_id+chunk_id+session_id+timestamp)
}

impl ChunkProofReq {
    pub fn sign(client_signer: &PeerSecret, source_device_id:&Peerid, miner_device_id:&Peerid, client_device_id:&Peerid, chunk_id:&Chunkid, session_id: &i64, timestamp: &Timespec)->BuckyResult<ChunkProofReq>{
        let buffer  = [
            source_device_id.to_string().as_bytes(),
            miner_device_id.to_string().as_bytes(),
            client_device_id.to_string().as_bytes(),
            chunk_id.to_string().as_bytes(),
            &session_id.to_ne_bytes(),
            &timestamp.sec.to_ne_bytes(),
            &timestamp.nsec.to_ne_bytes(),
        ].concat();

        let mut sign_bytes:[u8; SIGNATRUE_LENGTH] = [0u8; SIGNATRUE_LENGTH];
            
        client_signer.md5_sign(&buffer, &mut sign_bytes).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        let sign = sign_to_string(&sign_bytes);

        Ok(ChunkProofReq{
            source_device_id: source_device_id.clone(),
            miner_device_id: miner_device_id.clone(),
            client_device_id: client_device_id.clone(),
            chunk_id: chunk_id.clone(),
            session_id: session_id.clone(),
            timestamp_sec: timestamp.sec,
            timestamp_nsec: timestamp.nsec,
            proof: sign.to_owned()
        })
    }

    pub fn verify(&self, client_public_key: &PublicKey)->bool{

        let buffer  = [
            self.source_device_id.to_string().as_bytes(),
            self.miner_device_id.to_string().as_bytes(),
            self.client_device_id.to_string().as_bytes(),
            self.chunk_id.to_string().as_bytes(),
            &self.session_id.to_ne_bytes(),
            &self.timestamp_sec.to_ne_bytes(),
            &self.timestamp_nsec.to_ne_bytes(),
        ].concat();

        let ret = sign_from_string(&self.proof);
        if let Err(_e) = ret {
            return false;
        }
        let mut sign_bytes = ret.unwrap();
            
        return client_public_key.verify_md5(&buffer, &mut sign_bytes);
    }

    pub fn verify_redeem(client_public_key: &PublicKey, redeem_req: &ChunkRedeemReq)->bool{

        let buffer  = [
            redeem_req.source_device_id.to_string().as_bytes(),
            redeem_req.miner_device_id.to_string().as_bytes(),
            redeem_req.client_device_id.to_string().as_bytes(),
            redeem_req.chunk_id.to_string().as_bytes(),
            &redeem_req.session_id.to_ne_bytes(),
            &redeem_req.timestamp_sec.to_ne_bytes(),
            &redeem_req.timestamp_nsec.to_ne_bytes(),
        ].concat();

        let ret = sign_from_string(&redeem_req.proof);
        if let Err(_e) = ret {
            return false;
        }
        let mut sign_bytes = ret.unwrap();
            
        return client_public_key.verify_md5(&buffer, &mut sign_bytes);
    }
}
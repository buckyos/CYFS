use serde::{Serialize, Deserialize};
use ffs_base::{Peerid, Chunkid, BuckyResult, BuckyError, PeerSecret, PublicKey, SIGNATRUE_LENGTH};
use bdt2::crypto_tool::{PeerSecretImpl, PublicKeyImpl};
use time::Timespec;
use log::*;

use crate::ChunkProofReq;
use crate::chunk_base::*;

// Miner提交证明兑换信息
#[derive(Serialize, Deserialize)]
pub struct ChunkRedeemReq {
    pub source_device_id: Peerid, 
    pub miner_device_id: Peerid, 
    pub client_device_id: Peerid,
    pub chunk_id: Chunkid,
    pub session_id: i64,
    pub timestamp_sec: i64,
    pub timestamp_nsec: i32,
    pub proof: String,
    pub sign: String,
}

impl ChunkRedeemReq {
    pub fn sign(miner_signer: &PeerSecret, source_device_id:&Peerid, miner_device_id:&Peerid, client_device_id:&Peerid, chunk_id:&Chunkid, session_id:&i64, proofed_at:& Timespec, proof:&str)->BuckyResult<ChunkRedeemReq>{
        let buffer  = [
            source_device_id.to_string().as_bytes(),
            miner_device_id.to_string().as_bytes(),
            client_device_id.to_string().as_bytes(),
            chunk_id.to_string().as_bytes(),
            &session_id.to_ne_bytes(),
            &proofed_at.sec.to_ne_bytes(),
            &proofed_at.nsec.to_ne_bytes(),
            proof.as_bytes(),
        ].concat();

        let mut sign_bytes:[u8; SIGNATRUE_LENGTH] = [0u8; SIGNATRUE_LENGTH];
            
        miner_signer.md5_sign(&buffer, &mut sign_bytes).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        let sign = sign_to_string(&sign_bytes);

        Ok(ChunkRedeemReq{
            source_device_id: source_device_id.clone(),
            miner_device_id: miner_device_id.clone(),
            client_device_id: client_device_id.clone(),
            chunk_id: chunk_id.clone(),
            session_id: session_id.clone(),
            timestamp_sec: proofed_at.sec,
            timestamp_nsec: proofed_at.nsec,
            proof: proof.to_string(),
            sign: sign.to_owned()
        })
    }

    pub fn verify(&self, miner_public_key: &PublicKey, client_public_key: &PublicKey)->bool{
        // verify miner sign
        if !self.verify_sign(miner_public_key) {
            error!("ChunkRedeemReq verify sign failed");
            return false;
        }

        // verify client proof
        if !ChunkProofReq::verify_redeem(client_public_key, self) {
            error!("ChunkRedeemReq verify redeem failed");
            return false;
        }

        return true;
    }

    fn verify_sign(&self, miner_public_key: &PublicKey)->bool {
        let buffer  = [
            self.source_device_id.to_string().as_bytes(),
            self.miner_device_id.to_string().as_bytes(),
            self.client_device_id.to_string().as_bytes(),
            self.chunk_id.to_string().as_bytes(),
            &self.session_id.to_ne_bytes(),
            &self.timestamp_sec.to_ne_bytes(),
            &self.timestamp_nsec.to_ne_bytes(),
            self.proof.as_bytes(),
        ].concat();

        let ret = sign_from_string(&self.sign);
        if let Err(_e) = ret {
            return false;
        }
        let mut sign_bytes = ret.unwrap();
            
        return miner_public_key.verify_md5(&buffer, &mut sign_bytes);
    }
}
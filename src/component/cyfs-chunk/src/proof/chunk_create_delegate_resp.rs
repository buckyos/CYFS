use serde::{Serialize, Deserialize};
use ffs_base::{Peerid, Chunkid, BuckyResult, BuckyError, PeerSecret, PublicKey, SIGNATRUE_LENGTH};
use bdt2::crypto_tool::{PeerSecretImpl, PublicKeyImpl};
use cyfs_base_meta::TxHash;
use crate::chunk_base::*;

#[derive(Serialize, Deserialize)]
pub enum ChunkCreateDelegateStatus {
    Init = 0,
    UnionAccount = 1,
    Delegating = 2,
    Delegated = 3,
}

fn status_2_int(status: &ChunkCreateDelegateStatus)->i32{
    match status {
        ChunkCreateDelegateStatus::Init=>0,
        ChunkCreateDelegateStatus::UnionAccount=>1,
        ChunkCreateDelegateStatus::Delegating=>2,
        ChunkCreateDelegateStatus::Delegated=>3,
    }
}

#[derive(Serialize, Deserialize)]
pub struct ChunkCreateDelegateResp {
    pub source_device_id: Peerid,
    pub miner_device_id: Peerid,
    pub chunk_id: Chunkid,
    pub price: i64,
    pub status: ChunkCreateDelegateStatus,
    pub tx_hash: TxHash,
    pub sign: String,
}

impl ChunkCreateDelegateResp {
    pub fn sign(source_signer: &PeerSecret, source_device_id:&Peerid,  miner_device_id:&Peerid, chunk_id:&Chunkid, price:&i64, status: ChunkCreateDelegateStatus, tx_hash: TxHash)->BuckyResult<ChunkCreateDelegateResp>{

        let status_int = status_2_int(&status);

        let buffer  = [
            source_device_id.to_string().as_bytes(),
            miner_device_id.to_string().as_bytes(),
            chunk_id.to_string().as_bytes(),
            &price.to_ne_bytes(),
            &status_int.to_ne_bytes(),
            tx_hash.as_slice()
        ].concat();

        let mut sign_bytes:[u8; SIGNATRUE_LENGTH] = [0u8; SIGNATRUE_LENGTH];

        source_signer.md5_sign(&buffer, &mut sign_bytes).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        let sign = sign_to_string(&sign_bytes);

        Ok(ChunkCreateDelegateResp{
            source_device_id: source_device_id.clone(),
            miner_device_id: miner_device_id.clone(),
            chunk_id: chunk_id.clone(),
            price: price.clone(),
            status: status,
            tx_hash: tx_hash,
            sign: sign.to_owned()
        })
    }

    pub fn verify(&self, source_public_key:&PublicKey)->bool{

        let status_int = status_2_int(&self.status);

        let buffer  = [
            self.source_device_id.to_string().as_bytes(),
            self.miner_device_id.to_string().as_bytes(),
            self.chunk_id.to_string().as_bytes(),
            &self.price.to_ne_bytes(),
            &status_int.to_ne_bytes(),
            self.tx_hash.as_slice(),
        ].concat();

        let ret = sign_from_string(&self.sign);
        if let Err(_e) = ret {
            return false;
        }
        let mut sign_bytes = ret.unwrap();

        return source_public_key.verify_md5(&buffer, &mut sign_bytes);
    }
}

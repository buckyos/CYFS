use serde::{Serialize, Deserialize};
use ffs_base::{Peerid, Chunkid, BuckyResult, BuckyError, PeerSecret, PublicKey, SIGNATRUE_LENGTH};
use cyfs_base_meta::SignedTx;
use bdt2::crypto_tool::{PeerSecretImpl, PublicKeyImpl};
use crate::chunk_base::*;


// Miner提交证明兑换信息
#[derive(Serialize, Deserialize)]
pub struct ChunkRedeemResp {
    pub source_device_id: Peerid,
    pub miner_device_id: Peerid,
    pub client_device_id: Peerid,
    pub chunk_id: Chunkid,
    pub signed_tx: SignedTx,
    pub sign: String,
}

impl ChunkRedeemResp {
    pub fn sign(source_signer: &PeerSecret, source_device_id:&Peerid, miner_device_id:&Peerid, client_device_id:&Peerid, chunk_id:&Chunkid, signed_tx:SignedTx)->BuckyResult<ChunkRedeemResp>{

        let sign_tx_str = serde_json::ser::to_string(&signed_tx).map_err(|e| {
            BuckyError::from(e)
        })?;

        let buffer  = [
            source_device_id.to_string().as_bytes(),
            miner_device_id.to_string().as_bytes(),
            client_device_id.to_string().as_bytes(),
            chunk_id.to_string().as_bytes(),
            sign_tx_str.as_bytes(),
        ].concat();

        let mut sign_bytes:[u8; SIGNATRUE_LENGTH] = [0u8; SIGNATRUE_LENGTH];

        source_signer.md5_sign(&buffer, &mut sign_bytes).map_err(|_e|{
            BuckyError::from("sign chunk redirect failed")
        })?;

        let sign = sign_to_string(&sign_bytes);

        Ok(ChunkRedeemResp{
            source_device_id: source_device_id.clone(),
            miner_device_id: miner_device_id.clone(),
            client_device_id: client_device_id.clone(),
            chunk_id: chunk_id.clone(),
            signed_tx: signed_tx,
            sign: sign.to_owned(),
        })
    }

    pub fn verify(&self, source_public_key: &PublicKey)->bool{
        let sign_tx_str_ret = serde_json::ser::to_string(&self.signed_tx);
        if let Err(_e) = sign_tx_str_ret {
            return  false;
        }
        let sign_tx_str = sign_tx_str_ret.unwrap();

        let buffer  = [
            self.source_device_id.to_string().as_bytes(),
            self.miner_device_id.to_string().as_bytes(),
            self.client_device_id.to_string().as_bytes(),
            self.chunk_id.to_string().as_bytes(),
            sign_tx_str.as_bytes(),
        ].concat();

        let ret = sign_from_string(&self.sign);
        if let Err(_e) = ret {
            return false;
        }
        let mut sign_bytes = ret.unwrap();

        return source_public_key.verify_md5(&buffer, &mut sign_bytes);
    }
}

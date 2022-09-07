use async_std::io::{BufRead};
use async_std::prelude::*;
use log::*;

use cyfs_base::*;
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use cyfs_base_meta::*;
// use chunk_client::{ChunkClient, ChunkCacheContext};

use crate::chunk_store::ChunkStore;
use crate::chunk_context::ChunkContext;

pub struct ChunkManager {
    chunk_store: ChunkStore,
    meta_client: MetaClient,
    ctx: ChunkContext,
    device_id: DeviceId
}

impl ChunkManager {
    pub fn new(ctx: &ChunkContext) -> ChunkManager {
        ChunkManager{
            chunk_store: ChunkStore::new(&ctx.chunk_dir),
            meta_client: MetaClient::new_target(MetaMinerTarget::default()),
            ctx: ctx.clone(),
            device_id: ctx.get_device_id()
        }
    }

    pub fn get_device_id(&self) -> DeviceId{
        self.ctx.get_device_id()
    }

    pub fn get_public_key(&self,) -> &PublicKey{
        self.ctx.get_public_key()
    }

    pub fn get_private_key(&self) -> &PrivateKey{
        self.ctx.get_private_key()
    }

    pub async fn get(&self, chunk_id: &ChunkId) -> BuckyResult<impl BufRead + Unpin> {
        let reader = self.chunk_store.get(chunk_id).await.map_err(|e|{
            BuckyError::from(e)
        })?;

        Ok(reader)
    }

    pub fn delete(&self, chunk_id: &ChunkId) -> BuckyResult<()> {
        self.chunk_store.delete(chunk_id)
    }

    pub async fn set(&self, chunk_id: &ChunkId, data: &[u8])->BuckyResult<()>{
        info!("[set_chunk], begin set:{}", chunk_id);

        // let _ = self.chunk_store.set(chunk_id, data).await.map_err(|e|{
        //     BuckyError::from(e)
        // })?;

        let ret = self.chunk_store.set(chunk_id, data).await;

        match ret {
            Ok(_)=>{
                info!("[set_chunk], end set, success");
                Ok(())
            },
            Err(e)=>{
                info!("[set_chunk], end set fail");
                Err(BuckyError::from(e))
            }
        }
    }

    pub async fn create_union_account(&self, miner_device_id:&DeviceId, balance: &i64)->BuckyResult<CreateUnionTx>{
        let id = self.ctx.get_device().desc().calculate_id();
        let union_account = UnionAccount::new(id.clone(),
                                                 miner_device_id.object_id().clone(),
                                                 cyfs_meta_lib::UNION_ACCOUNT_TYPE_CHUNK_PROOF).build();
        let mut create_union_tx = CreateUnionTx::new(union_account, CoinTokenId::Coin(0));
        create_union_tx.set_recharge_amount(&id, *balance)?;
        create_union_tx.async_sign(ObjectLink{ obj_id: id.clone(), obj_owner: None}, self.ctx.get_private_key().clone()).await?;
        Ok(create_union_tx)
    }

    pub async fn delegate(&self, miner_device_id:&DeviceId, chunk_id:&ChunkId, _price:&i64)->BuckyResult<()>{
        let _source_peer_sec = self.get_private_key();
        let _source_device_id = &self.device_id;
        let miner_public_key = self.get_peer_public_key(miner_device_id).await?;

        info!("[{}] miner public key is:{:?}", chunk_id.to_string(), miner_public_key);

        let mut read = self.get(&chunk_id).await?;
        let mut data = Vec::new();
        let _ = read.read_to_end(&mut data).await.map_err(|e|{
            error!("[{}] read chunk data failed, msg:{}", chunk_id.to_string(), e.to_string());
            BuckyError::from(e)
        })?;

        // TODO:

        // let chunk_req = cyfs_chunk::ChunkDelegateReq::sign(
        //     &source_peer_sec,
        //     &miner_public_key,
        //     &source_device_id,
        //     &miner_device_id,
        //     &chunk_id,
        //     &price,
        //     data
        // ).map_err(|e|{
        //     error!("[{}] sign delegate req failed, msg:{}", chunk_id.to_string(), e.to_string());
        //     BuckyError::from(e)
        // })?;

        // let ctx = ChunkCacheContext::cache_http_local(&miner_device_id);
        // let chunk_delegate_resp = ChunkClient::delegate(ctx,&chunk_req).await.map_err(|e|{
        //     error!("[{}] delegate to cache miner failed, msg:{}", chunk_id.to_string(), e.to_string());
        //     BuckyError::from(e)
        // })?;

        // if !chunk_delegate_resp.verify(&miner_public_key) {
        //     error!("[{}] verify delegate resp from cache miner failed", chunk_id.to_string());
        //     return Err(BuckyError::from(BuckyErrorCode::ErrorState));
        // }

        Ok(())
    }

    pub async fn get_peer_public_key(&self, device_id: &DeviceId) -> BuckyResult<cyfs_base::PublicKey>{

        // TODO: 使用缓存优化
        let desc = self.meta_client.get_desc(device_id.object_id()).await;
        let result = match desc {
            Ok(desc_obj) => {
                let key = match desc_obj {
                    SavedMetaObject::Device(device) => Ok(device.desc().public_key().clone()),
                    _ => Err(BuckyError::from("peer desc not found")),
                };
                key
            },
            _ => Err(BuckyError::from("peer desc not found")),
        };

        result
    }

    pub async fn sign_trans(&self, miner_device_id:&DeviceId, seq:i64, deviation: i64) -> BuckyResult<DeviateUnionTx> {
        let signer = self.ctx.get_private_key();
        let union_desc = UnionAccount::new(
            self.device_id.object_id().clone(),
            miner_device_id.object_id().clone(),
            cyfs_meta_lib::UNION_ACCOUNT_TYPE_CHUNK_PROOF
        ).build();

        let union_id = union_desc.desc().calculate_id();
        let ctid = CoinTokenId::Coin(0);
        let mut deviate = DeviateUnionTx::new(union_id.clone(), ctid, seq, deviation);
        deviate.async_sign(ObjectLink {obj_id: union_id, obj_owner: None}, signer.clone()).await?;

        Ok(deviate)
    }
}

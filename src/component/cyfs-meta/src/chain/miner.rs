use cyfs_base::{BuckyResult, ObjectId};
use crate::Chain;
use async_trait::async_trait;
use std::sync::{Arc, Weak};
use cyfs_base_meta::MetaTx;

#[async_trait]
pub trait Miner: Sync + Send {
    fn as_chain(&self) -> &Chain;
    async fn push_tx(&self, tx: MetaTx) -> BuckyResult<()>;
    async fn get_nonce(&self, account: &ObjectId) -> BuckyResult<i64>;
    fn get_interval(&self) -> u64;
}

pub trait MinerRuner {
    fn run(self: &Arc<Self>) -> BuckyResult<()>;
}

pub type MinerRef = Arc<dyn Miner>;
pub type MinerWeakRef = Weak<dyn Miner>;

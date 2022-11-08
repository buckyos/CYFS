use crate::{Chain};
use std::ops::{Deref, DerefMut};
use std::path::Path;
use crate::state_storage::StorageRef;
use crate::archive_storage::ArchiveStorageRef;
use cyfs_base::*;
use crate::chain::{Miner, BaseMiner, MinerRuner};
use std::thread;
use std::time::Duration;
use log::*;
use async_trait::async_trait;
use std::sync::Arc;
use cyfs_base_meta::MetaTx;

pub struct StandaloneMiner {
    base: BaseMiner,
}

#[async_trait]
impl Miner for StandaloneMiner {
    fn as_chain(&self) -> &Chain {
        self.base.as_chain()
    }

    async fn push_tx(&self, tx: MetaTx) -> BuckyResult<()> {
        self.base.push_tx(tx).await
    }

    async fn get_nonce(&self, account: &ObjectId) -> BuckyResult<i64> {
        self.base.get_nonce(account).await
    }

    fn get_interval(&self) -> u64 {
        self.base.interval() as u64
    }
}

impl MinerRuner for StandaloneMiner {
    fn run(self: &Arc<Self>) -> BuckyResult<()> {
        let miner = self.clone();
        thread::spawn(move || {
            loop {
                if let Err(e) = miner.mine_block() {
                    error!("mine block error! err={}", e);
                }
                thread::sleep(Duration::from_secs(miner.interval() as u64));
            }
        });
        Ok(())
    }
}

impl StandaloneMiner {
    pub fn new(coinbase: ObjectId, interval: u32, chain: Chain, bfc_spv_node: String) -> BuckyResult<Self> {
        Ok(StandaloneMiner {
            base: BaseMiner::new(coinbase, interval, chain, bfc_spv_node, None),
        })
    }

    pub async fn load(coinbase: ObjectId, interval: u32, bfc_spv_node: String, dir: &Path, new_storage: fn (path: &Path) -> StorageRef, archive_storage: fn (path: &Path) -> ArchiveStorageRef) -> BuckyResult<Self> {
        let chain = Chain::load(dir, new_storage, archive_storage).await?;
        Ok(StandaloneMiner {
            base: BaseMiner::new(coinbase, interval, chain, bfc_spv_node, None)
        })
    }
}

impl Deref for StandaloneMiner {
    type Target = BaseMiner;

    fn deref(&self) -> &Self::Target {
        &self.base
    }
}

impl DerefMut for StandaloneMiner {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base
    }
}

#[cfg(test)]
mod test {
    use cyfs_base_meta::*;
    use crate::{new_sql_storage, BlockDesc, State};
    use cyfs_base::{ObjectId, HashValue};
    use crate::chain::{BlockExecutor};
    use crate::executor::context::Config;
    use crate::mint::btc_mint::BTCMint;
    use std::fs::{create_dir, remove_dir_all};

    #[test]
    fn test() {
        async_std::task::block_on(async {
            let config = GenesisConfig {
                chain_type: Some("".to_string()),
                coinbase: Default::default(),
                interval: 0,
                bfc_spv_node: "".to_string(),
                coins: vec![],
                price: GenesisPriceConfig {},
                miner_key_path: None,
                mg_path: None,
                miner_desc_path: None,
                sub_chain_tx: None
            };

            let mut temp_dir = std::env::temp_dir();
            temp_dir.push("rust_test");
            if temp_dir.exists() {
                remove_dir_all(temp_dir.clone()).unwrap();
            }
            create_dir(temp_dir.clone()).unwrap();

            let header = BlockDesc::new(BlockDescContent::new(ObjectId::default(), None)).build();
            let mut block_body = BlockBody::new();
            let mut genesis_storage = temp_dir.clone();
            genesis_storage.push("genesis");
            let storage = new_sql_storage(genesis_storage.as_path());
            let state = storage.create_state(false).await;
            state.init_genesis(&config.coins).await.unwrap();
            let meta_config = Config::new(&state).unwrap();
            state.create_cycle_event_table(meta_config.get_rent_cycle()).await.unwrap();

            let _btc_mint = BTCMint::new(&state, &meta_config, config.bfc_spv_node.as_str());
            // if let Ok(coinage_tx) = btc_mint.create_btc_genesis_tx() {
            //     let tx = MetaTx::new(1, TxCaller::Miner(ObjectId::default()), 0, 0, 0
            //                      , None, TxBody::BTCCoinageRecord(coinage_tx), Vec::new()).build();
            //     block.add_transaction(tx).unwrap();
            // }

            BlockExecutor::execute_block(&header, &mut block_body, &state, &meta_config, None, "".to_string(), None, ObjectId::default()).await.unwrap();
            let _block = Block::new(ObjectId::default(), None, HashValue::default(), block_body).unwrap().build();
            // let chain = Chain::new(temp_dir.as_path(), new_sql_storage, block, &storage).await.unwrap();
            // let ret = StandaloneMiner::new(ObjectId::default(),  0, chain, "".to_string());
            //
            // let chain = ret.unwrap().as_chain();
        });
    }
}

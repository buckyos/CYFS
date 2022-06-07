use cyfs_base::BuckyResult;
use cyfs_base_meta::{Block, BlockTrait, BlockDescTrait};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use std::time::Duration;
use crate::SPVChainStorageRef;
use std::str::FromStr;
use async_std::prelude::*;

pub struct BlockMonitor {
    meta_client: MetaClient,
    chain_storage: SPVChainStorageRef,
}

impl BlockMonitor {
    pub fn new(meta_host: &str, chain_storage: SPVChainStorageRef) -> Self {
        Self {
            meta_client: MetaClient::new_target(MetaMinerTarget::from_str(meta_host).unwrap()),
            chain_storage,
        }
    }

    pub async fn get_cur_block_height(&self) -> BuckyResult<i64> {
        let chain_status = self.meta_client.get_chain_status().await?;
        Ok(chain_status.height)
    }

    async fn get_local_block_height(&self) -> BuckyResult<i64> {
        self.chain_storage.get_local_block_height().await
    }

    pub async fn on_new_block(&self, block: Block) -> BuckyResult<()> {
        self.chain_storage.add_mined_block(block).await
    }

    pub async fn run(self) {
        async_std::task::spawn(async move {
            let mut interval = async_std::stream::interval(Duration::from_secs(10));
            while let Some(_) = interval.next().await {
                loop {
                    let cur_height = self.get_cur_block_height().await;
                    let local_height = match self.get_local_block_height().await {
                        Ok(height) => {
                            height
                        }
                        Err(_) => {
                            -1
                        }
                    };
                    if cur_height.is_ok() {
                        for i in local_height + 1..cur_height.unwrap() - 2 {
                            let block = self.meta_client.get_block(i).await;
                            if block.is_err() {
                                break;
                            }
                            let block = block.unwrap();
                            log::info!("get block {} height {}", block.header().hash().to_string(), i);

                            let ret = self.on_new_block(block).await;
                            if ret.is_err() {
                                log::error!("on_new_block err {}", ret.err().unwrap());
                                break;	
                            }
                        }
                    }
                    async_std::task::sleep(Duration::new(10, 0)).await
                }
            }
        });

    }

}

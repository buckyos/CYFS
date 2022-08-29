use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use crate::storage::Storage;
use std::sync::{Arc};
use std::str::FromStr;
use std::time::Duration;
use cyfs_base::{BuckyResult};
use log::*;
use cyfs_base_meta::{Block, BlockTrait, BlockDescTrait};
use async_std::stream::StreamExt;
use crate::status::Status;

pub struct BlockMonitor {
    meta_client: MetaClient,
    storage: Arc<Box<dyn Storage + Send + Sync>>,
    status: Arc<Status>
}

impl BlockMonitor {
    pub fn new(meta_endpoint: &str, storage: Arc<Box<dyn Storage + Send + Sync>>, status: Arc<Status>) -> BlockMonitor {
        BlockMonitor {
            meta_client: MetaClient::new_target(MetaMinerTarget::from_str(meta_endpoint).unwrap()),
            storage,
            status
        }
    }

    pub async fn init(&self) -> BuckyResult<()> {
        let cur_height = self.storage.get_cur_height().await?;
        info!("cur height: {}", cur_height);
        self.status.set_height(cur_height);
        self.status.set_tx_num(self.storage.get_tx_sum().await?);
        Ok(())
    }

    async fn meta_height(&self) -> BuckyResult<i64> {
        Ok(self.meta_client.get_chain_status().await?.height)
    }

    async fn parse_block(&self, block: Block) -> BuckyResult<()> {
        // 先存储block信息
        match self.storage.add_block(&block).await {
            Ok(size) => {
                self.status.add_tx_num(size as u64);
                self.status.set_height(block.header().number());
                Ok(())
            }
            Err(e) => {
                error!("storage add block {} info err {}", block.header().number(), e);
                Err(e)
            }
        }
    }

    pub fn run(monitor: Arc<BlockMonitor>, interval: u64) {
        let monitor1 = monitor.clone();
        async_std::task::spawn(async move {
            let mut interval = async_std::stream::interval(Duration::from_secs(interval));
            while let Some(_) = interval.next().await {
                let cur_height = monitor1.status.cur_height();
                if let Ok(meta_height) = monitor1.meta_height().await {
                    if cur_height < meta_height - 2 {
                        for i in cur_height + 1..meta_height - 2 {
                            info!("syncing block, cur {}, target {}", i, meta_height - 2);
                            match monitor1.meta_client.get_block(i).await {
                                Ok(block) => {
                                    if let Err(e) = monitor1.parse_block(block).await {
                                        error!("parse block {} err {}", i, e);
                                        break;
                                    }
                                }
                                Err(e) => {
                                    error!("get block {} from meta err {}, skip syncing", i, e);
                                    continue;
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}
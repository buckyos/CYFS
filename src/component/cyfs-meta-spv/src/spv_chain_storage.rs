use std::path::{Path, PathBuf};
use cyfs_base::{BuckyResult};
use std::sync::{Arc, Weak, Mutex};
use cyfs_base_meta::{Block};
use std::fs::remove_file;
use crate::spv_tx_storage::*;
use crate::db_helper::*;
use sqlx::*;
use log::LevelFilter;
use std::time::Duration;
use std::future::Future;
use async_trait::async_trait;
use sqlx::sqlite::SqliteJournalMode;
use cyfs_base_meta::*;
use crate::NFTStorage;

pub type SPVChainStorageRef = Arc<SPVChainStorage>;
pub type SPVChainStorageWeakRef = Weak<SPVChainStorage>;

#[async_trait]
pub trait BlockEventEndpoint: 'static + Send + Sync {
    async fn on_new_block(&self, chain_storage: SPVTxStorageRef, block: Block) -> BuckyResult<()>;
}

#[async_trait]
impl<F, Fut> BlockEventEndpoint for F
    where
        F: Send + Sync + 'static + Fn(SPVTxStorageRef, Block) -> Fut,
        Fut: Send + 'static + Future<Output = BuckyResult<()>>,
{
    async fn on_new_block(&self, chain_storage: SPVTxStorageRef, block: Block) -> BuckyResult<()> {
        let fut = (self)(chain_storage, block);
        fut.await
    }
}

pub struct SPVChainStorage {
    db_path: PathBuf,
    listener: Mutex<Option<Arc<dyn BlockEventEndpoint>>>,
}

impl SPVChainStorage {
    pub async fn load(dir: &Path) -> BuckyResult<SPVChainStorageRef> {
        let chain_storage = Arc::new(Self {
            db_path: dir.join("spv_db"),
            listener: Mutex::new(None),
        });
        let tx_storage = chain_storage.create_tx_storage().await?;
        tx_storage.init().await?;
        tx_storage.init_nft_storage().await?;
        Ok(chain_storage)
    }

    pub async fn reset(dir: &Path, block: Option<&Block>) -> BuckyResult<SPVChainStorageRef> {
        // assert_eq!(block.header().number(), 0);
        let spv_db = dir.join("spv_db");
        if spv_db.exists() {
            remove_file(dir.join("spv_db"))?;
        }
        let storage = Self {
            db_path: dir.join("spv_db"),
            listener: Mutex::new(None),
        };

        let tx_storage = storage.create_tx_storage().await?;
        tx_storage.init().await?;

        if block.is_some() {
            tx_storage.add_block(block.as_ref().unwrap()).await?;
        }
        Ok(Arc::new(storage))
    }

    pub async fn create_tx_storage(&self) -> BuckyResult<SPVTxStorageRef> {
        let mut options = MetaConnectionOptions::new().filename(self.db_path.as_path()).create_if_missing(true)
            .journal_mode(SqliteJournalMode::Memory).busy_timeout(Duration::new(10, 0));
        options.log_statements(LevelFilter::Off).log_slow_statements(LevelFilter::Off, Duration::new(10, 0));
        let conn = options.connect().await.map_err(map_sql_err)?;
        Ok(SPVTxStorage::new(conn))
    }

    pub async fn add_mined_block(&self, block: Block) -> BuckyResult<()> {
        log::info!("add_mined_block start");
        let tx_storage = self.create_tx_storage().await?;
        tx_storage.being_transaction().await?;

        let tmp_tx_storage = tx_storage.clone();
        let ret: BuckyResult<()> = async move {
            //spv node header
            tmp_tx_storage.save_header(block.header()).await?;
            tmp_tx_storage.change_tip(block.header()).await?;
            tmp_tx_storage.add_block(&block).await?;
            let listener = {
                let listener = self.listener.lock().unwrap();
                if listener.is_some() {
                    Some(listener.as_ref().unwrap().clone())
                } else {
                    None
                }
            };
            if listener.is_some() {
                listener.unwrap().on_new_block(tmp_tx_storage, block).await?;
            }
            Ok(())
        }.await;
        if ret.is_err() {
            tx_storage.rollback().await?;
            log::info!("add_mined_block rollback");
            Err(ret.err().unwrap())
        } else {
            tx_storage.commit().await?;
            log::info!("add_mined_block commit");
            Ok(())
        }
    }

    pub fn set_block_listener(&self, listener: impl BlockEventEndpoint) {
        let mut listener_lock = self.listener.lock().unwrap();
        *listener_lock = Some(Arc::new(listener));
    }

    pub async fn get_local_block_height(&self) -> BuckyResult<i64> {
        let tx_storage = self.create_tx_storage().await?;
        let height = tx_storage.config_get("latest_height", Some("-1".to_string())).await?;
        if height == "-1".to_owned() {
            return Err(crate::meta_err!(ERROR_INVALID));
        }

        Ok(height.parse::<i64>().unwrap())
    }
}

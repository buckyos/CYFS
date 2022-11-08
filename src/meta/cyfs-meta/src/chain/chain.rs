use cyfs_base_meta::*;
use cyfs_base::*;
use crate::archive_storage::ArchiveStorageRef;
use crate::state_storage::StorageRef;
use crate::chain::chain_storage::ChainStorageRef;
use std::path::{Path, PathBuf};
use crate::chain::{ChainStorage};

pub fn to_meta_data(tx: &MetaTx, receipt: &Receipt) -> BuckyResult<TxMetaData> {
    let mut trans_list = Vec::<(String, u8, String)>::new();
    let bodys = tx.desc().content().body.get_obj();
    for body in bodys {
        match body {
            MetaTxBody::TransBalance(tx) => {
                for (dest, v) in &tx.to {
                    trans_list.push((dest.to_string(), 0, (*v).to_string()))
                }
            }
            MetaTxBody::WithdrawToOwner(withdraw) => {
                trans_list.push((tx.desc().content().caller.id()?.to_string(), 0, withdraw.value.to_string()))
            }
            _ => {
                // do nothing
            }
        }
    }

    Ok(TxMetaData {
        tx_hash: tx.desc().calculate_id().to_string(),
        create_time: bucky_time_to_js_time(tx.desc().create_time()).to_string(),
        nonce: tx.desc().content().nonce.to_string(),
        caller: tx.desc().content().caller.id()?.to_string(),
        gas_coin_id: tx.desc().content().gas_coin_id,
        gas_price: tx.desc().content().gas_price,
        max_fee: tx.desc().content().max_fee,
        use_fee: receipt.fee_used,
        result: receipt.result,
        to: trans_list
    })
}

pub struct Chain {
    storage: ChainStorageRef,
}

impl Chain {
    pub async fn new(dir: PathBuf, block: Option<Block>, storage: StorageRef, archive_storage: ArchiveStorageRef) -> BuckyResult<Self> {
        let chain_storage = ChainStorage::reset(dir, block, storage, archive_storage).await?;
        Ok(Self {
            storage: chain_storage
        })
    }

    pub async fn load(dir: &Path, new_storage: fn (path: &Path) -> StorageRef, archive_storage: fn (path: &Path) -> ArchiveStorageRef) -> BuckyResult<Self> {
        let chain_storage = ChainStorage::load(dir, new_storage, archive_storage).await?;
        let ret = chain_storage.get_tip_info().await;
        if ret.is_ok() {
            let (_tip_header, _, _) = ret.unwrap();
        }
        Ok(Self {
            storage: chain_storage
        })
    }

    pub fn get_chain_storage(&self) -> &ChainStorageRef {
        &self.storage
    }
    pub async fn add_mined_block(&self, block: &Block) -> BuckyResult<()> {
        self.storage.add_mined_block(block).await
    }

    pub async fn backup(&self, height: i64) -> BuckyResult<()> {
        self.storage.backup(height).await
    }

    pub async fn recovery(&self, height: i64) -> BuckyResult<()> {
        self.storage.recovery(height).await
    }
}

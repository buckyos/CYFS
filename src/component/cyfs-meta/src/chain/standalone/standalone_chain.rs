use crate::{Block};
use crate::chain::{Chain, ChainStorage, ChainStatus, TxInfo};
use crate::state_storage::{Snapshot, StorageManager, StorageRef};
use cyfs_base_meta::{ViewBlockEnum, MetaTx, TxHash, BlockHeader, ViewRequest, ViewResponse};
use cyfs_base::{ObjectId, BuckyResult, Receipt};
use std::path::Path;
use std::sync::Mutex;
use crate::chain::pending::PendingTransactions;
use crate::chain::storage::chain_storage::ChainStorageRef;

pub struct StandaloneChain {
    storage: ChainStorageRef,
    pending: Mutex<PendingTransactions>
}

impl StandaloneChain {
    pub fn new(dir: &Path, new_storage: fn (path: &Path) -> StorageRef, block: Block, storage: &StorageRef) -> BuckyResult<StandaloneChain> {
        let chain_storage = ChainStorage::reset(dir, new_storage, block, storage)?;
        Ok(StandaloneChain {
            storage: chain_storage,
            pending: Mutex::new(PendingTransactions::new(&chain_storage))
        })
    }

    pub fn load(dir: &Path, new_storage: fn (path: &Path) -> StorageRef) -> BuckyResult<Self> {
        let chain_storage = ChainStorage::load(dir, new_storage)?;
        Ok(StandaloneChain {
            storage: chain_storage,
            pending: Mutex::new(PendingTransactions::new(&chain_storage))
        })
    }
}

impl StandaloneChain {
    fn block_header(&self, block: ViewBlockEnum) -> BuckyResult<BlockHeader> {
        self.storage.block_header(block)
    }

    fn add_mined_block(&self, block: &Block, snapshot: &Snapshot) -> BuckyResult<()> {
        self.storage.add_mined_block(block, snapshot)
    }

    fn storage_manager(&self) -> &StorageManager {
        self.storage.storage_manager()
    }

    // thread safe
    // lock pending
    fn commit(&self, tx: MetaTx) -> BuckyResult<()> {
        let mut pending = self.pending.lock().unwrap();
        pending.push(tx)
    }

    // thread safe
    // lock pending
    fn pop_all_pending(&self) -> BuckyResult<Vec<MetaTx>> {
        let mut pending = self.pending.lock().unwrap();
        pending.pop_all()
    }

    fn tx_exists_in_pending(&self, hash: &TxHash) -> bool {
        let pending = self.pending.lock().unwrap();
        pending.exists(hash)
    }

    // thread safe
    fn nonce_of(&self, account: &ObjectId) -> BuckyResult<i64> {
        // let state = self.block_state(ViewBlockEnum::Tip)?;
        // state.get_nonce(account)
        let pending = self.pending.lock().unwrap();
        pending.get_nonce(account)
    }

    fn receipt_of(&self, tx_hash: &TxHash) -> BuckyResult<Option<(Receipt, i64)>> {
        self.storage.receipt_of(tx_hash)
    }

    fn view(&self, request: ViewRequest) -> BuckyResult<ViewResponse> {
        self.storage.view(request)
    }

    fn get_balance(&self, address_list: Vec<(u8, String)>) -> BuckyResult<Vec<i64>> {
        self.storage.get_balance(address_list)
    }

    fn get_status(&self) -> BuckyResult<ChainStatus> {
        self.storage.get_status()
    }

    fn get_tx_info(&self, tx_hash: TxHash) -> BuckyResult<TxInfo> {
        self.storage.get_tx_info(tx_hash)
    }
}

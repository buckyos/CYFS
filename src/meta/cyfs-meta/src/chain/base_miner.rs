use cyfs_base_meta::*;

use cyfs_base::*;

use crate::{state_storage::*, archive_storage::*};

use crate::executor::context::{Config, ConfigRef, UnionWithdrawManager};
use crate::rent::rent_manager::RentManager;
use crate::events::event_manager::EventManager;
use crate::executor::tx_executor::TxExecutor;
use crate::name_auction::auction::Auction;
use crate::mint::btc_mint::{BTCMint};
use crate::chain::{Chain, to_meta_data};
use crate::chain::pending::PendingTransactions;
use async_std::sync::{Mutex, MutexGuard};
use crate::{get_meta_err_code, NFTAuction};
use std::convert::TryFrom;
use crate::chain::storage::chain_storage::ChainStorageRef;

pub struct BaseMiner {
    coinbase: ObjectId,
    interval: u32,
    chain: Chain,
    bfc_spv_node: String,
    pending: Mutex<PendingTransactions>,
    miner_key: Option<PrivateKey>,
}

impl BaseMiner {
    pub fn new(coinbase: ObjectId, interval: u32, chain: Chain, bfc_spv_node: String, miner_key: Option<PrivateKey>) -> BaseMiner {
        BaseMiner {
            pending: Mutex::new(PendingTransactions::new(chain.get_chain_storage())),
            coinbase,
            interval,
            chain,
            bfc_spv_node,
            miner_key,
        }
    }

    pub fn as_chain(&self) -> &Chain {
        &self.chain
    }

    // thread safe
    pub async fn create_block(&self, transactions: Vec<MetaTx>) -> BuckyResult<Block> {
        let tip = self.chain.get_chain_storage().block_header(ViewBlockEnum::Tip).await?;
        let header = BlockDesc::new(
            BlockDescContent::new(self.coinbase.clone(), Some(&tip))).build();

        let storage = self.chain.get_chain_storage().state_storage();
        let mut block_body = BlockBody::new();
        let old_state_hash = storage.state_hash().await?;
        log::info!("miner begin create block number {:?} state_hash {}", header.number(), old_state_hash.to_string());
        {
            let ref_state = storage.create_state(false).await;
            let config = Config::new(&ref_state)?;
            for tx in transactions {
                block_body.add_transaction(tx.clone()).unwrap();
            }

            // ref_state.being_transaction().await?;
            let archive_storage = self.chain.get_chain_storage().archive_storage();
            let ref_archive = archive_storage.create_archive(false).await;
            let ret = BlockExecutor::execute_block(&header,
                                                   &mut block_body,
                                                   &ref_state,
                                                   &ref_archive,
                                                   &config,
                                                   Some(self.chain.get_chain_storage()),
                                                   self.bfc_spv_node.clone(),
                                                   self.miner_key.clone(),
                                                   self.coinbase.clone()).await;
            if ret.is_ok() {
                // ref_state.commit().await?;
            } else {
                // ref_state.rollback().await?;
                ret?;
            }
        }
        log::info!("start calculate state hash");
        let state_hash = storage.state_hash().await?;
        let block = Block::new(self.coinbase.clone(), Some(&tip), state_hash, block_body)?.build();
        log::info!("miner mined block {:?} state_hash {}", block.header().hash().to_string(), state_hash.to_string());
        Ok(block)
    }

    // thread safe
    pub fn mine_block(&self) -> BuckyResult<()> {
        async_std::task::block_on(async {
            let mut transactions = {
                let mut pending = self.get_tx_pending_list().await;
                pending.pop_all()?
            };
            let ref_state = self.as_chain().get_chain_storage().state_storage().create_state(false).await;
            let config = Config::new(&ref_state)?;
            let btc_mint = BTCMint::new(&ref_state, &config, self.bfc_spv_node.as_str());
            if let Ok(coinage_tx) = btc_mint.create_btc_coinage_record_tx() {
                let tx = MetaTx::new(1, TxCaller::Miner(ObjectId::default())
                                     , 0, 0, 0
                                     , None, MetaTxBody::BTCCoinageRecord(coinage_tx)
                                     , Vec::new()).build();
                transactions.push(tx);
            }
            let block = self.create_block(transactions).await?;
            self.chain.add_mined_block(&block).await?;
            Ok(())
        })
    }

    pub fn interval(&self) -> u32 {
        self.interval
    }

    pub fn coinbase(&self) -> &ObjectId {
        &self.coinbase
    }

    pub fn bfc_spv_node(&self) -> &str {
        self.bfc_spv_node.as_str()
    }


    pub async fn get_tx_info(&self, tx_hash: &TxHash) -> BuckyResult<TxInfo> {
        let ret = {
            let pending = self.pending.lock().await;
            pending.get_tx(tx_hash)
        };
        if ret.is_some() {
            Ok(TxInfo {
                status: TX_STATUS_PENDING,
                tx: to_meta_data(&ret.unwrap(), &Receipt::new(0,0))?,
                block_number: None,
                block_hash: None,
                block_create_time: None,
            })
        } else {
            self.chain.get_chain_storage().get_tx_info(tx_hash).await
        }
    }

    pub async fn get_tx_pending_list(&self) -> MutexGuard<'_, PendingTransactions> {
        self.pending.lock().await
    }

    pub async fn get_nonce(&self, account: &ObjectId) -> BuckyResult<i64> {
        self.get_tx_pending_list().await.get_nonce(account).await
    }

    pub async fn push_tx(&self, tx: MetaTx) -> BuckyResult<()> {
        self.get_tx_pending_list().await.push(tx).await
    }

    pub async fn remove(&self, tx: &MetaTx) -> BuckyResult<()> {
        self.get_tx_pending_list().await.remove(tx).await
    }
}

pub struct BlockExecutor {}

impl BlockExecutor {
    pub async fn execute_block(header: &BlockDesc, block: &mut BlockBody, ref_state: &StateRef, ref_archive: &ArchiveRef, config: &ConfigRef, chain_storage: Option<&ChainStorageRef>, mint_url: String, miner_key: Option<PrivateKey>, miner_id: ObjectId) -> BuckyResult<()> {
        // ref_state.being_transaction().await?;
        let ret = async {
            let event_manager = EventManager::new(&ref_state, &config);
            let rent_manager = RentManager::new(&ref_state, &config, &event_manager);
            let auction = Auction::new(&ref_state, &config, &rent_manager, &event_manager);
            let union_withdraw_manager = UnionWithdrawManager::new(&ref_state, &config, &event_manager);
            let nft_auction = NFTAuction::new(&ref_state, &config, &event_manager);
            let tx_executor = TxExecutor::new(
                ref_state,
                config,
                &rent_manager,
                &auction,
                &event_manager,
                &union_withdraw_manager,
                &nft_auction,
                ref_archive,
                mint_url,
                miner_key,
                miner_id,
                false);
            let mut receipts = vec![];
            header.calculate_id();
            for tx in block.transactions() {
                if !tx.desc().content().caller.is_miner() {
                    let public_key = {
                        let account_info_ret = ref_state.get_account_info(&tx.desc().content().caller.id()?).await;
                        if let Err(err) = &account_info_ret {
                            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                                let public_key = tx.desc().content().caller.get_public_key()?.clone();
                                ref_state.add_account_info(&AccountInfo::try_from(tx.desc().content().caller.clone())?).await?;
                                public_key
                            } else {
                                return Err(account_info_ret.err().unwrap());
                            }
                        } else {
                            account_info_ret.unwrap().get_public_key()?.clone()
                        }
                    };
                    if !tx.async_verify_signature(public_key).await? {
                        return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "InvalidInput"));
                    }
                }
                let receipt = tx_executor.execute(header, tx, chain_storage).await?;
                receipts.push(receipt);
            }
            block.add_receipts(receipts)?;
            //TODO: execute events

            let ret = event_manager.run_event(header).await;
            if ret.is_err() {
                log::error!("run event error:{}", ret.err().unwrap());
            } else {
                log::info!("event count {}", ret.as_ref().unwrap().len());
                block.set_event_records(ret.unwrap());
            }
            Ok(())
        }.await;

        // if ret.is_ok() {
        //     ref_state.commit().await?;
        // } else {
        //     ref_state.rollback().await?;
        // }
        ret
    }

    pub async fn execute_and_verify_block(block: &Block, storage: &StorageRef, archive_storage: &ArchiveStorageRef, chain_storage: Option<&ChainStorageRef>, mint_url: &str, miner_key: Option<PrivateKey>, miner_id: ObjectId) -> BuckyResult<bool> {
        let (block_desc, block_body) = {
            log::info!("old_state_hash {}", storage.state_hash().await?.to_string());
            let ref_state = storage.create_state(false).await;
            let config = Config::new(&ref_state)?;
            let event_manager = EventManager::new(&ref_state, &config);
            let rent_manager = RentManager::new(&ref_state, &config, &event_manager);
            let auction = Auction::new(&ref_state, &config, &rent_manager, &event_manager);
            let union_withdraw_manager = UnionWithdrawManager::new(&ref_state, &config, &event_manager);
            let nft_auction = NFTAuction::new(&ref_state, &config, &event_manager);
            let ref_archive = archive_storage.create_archive(false).await;
            let tx_executor = TxExecutor::new(
                &ref_state,
                &config,
                &rent_manager,
                &auction,
                &event_manager,
                &union_withdraw_manager,
                &nft_auction,
                &ref_archive,
                mint_url.to_owned(),
                miner_key,
                miner_id,
                true);
            let block_desc = block.desc();
            let mut block_body = BlockBody::new();
            let mut receipts = Vec::new();
            for tx in block.transactions() {
                if !tx.desc().content().caller.is_miner() {
                    let public_key = {
                        let account_info_ret = ref_state.get_account_info(&tx.desc().content().caller.id()?).await;
                        if let Err(err) = &account_info_ret {
                            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                                let public_key = tx.desc().content().caller.get_public_key()?.clone();
                                ref_state.add_account_info(&AccountInfo::try_from(tx.desc().content().caller.clone())?).await?;
                                public_key
                            } else {
                                return Err(account_info_ret.err().unwrap());
                            }
                        } else {
                            account_info_ret.unwrap().get_public_key()?.clone()
                        }
                    };
                    if !tx.async_verify_signature(public_key).await? {
                        return Ok(false);
                    }
                }
                block_body.add_transaction(tx.clone())?;
                let receipt = tx_executor.execute(block_desc, tx, chain_storage).await?;
                receipts.push(receipt);
            }
            block_body.add_receipts(receipts)?;

            let ret = event_manager.run_event(block_desc).await;
            if ret.is_err() {
                log::error!("run event error:{}", ret.err().unwrap());
            } else {
                log::info!("event count {}", ret.as_ref().unwrap().len());
                block_body.set_event_records(ret.unwrap());
            }

            (block_desc, block_body)
        };

        let state_hash = storage.state_hash().await?;

        let new_block = Block::new2(block_desc,
                    state_hash,
                    block_body)?.build();
        log::info!("state_hash old:{} new:{}", block.desc().state_hash().to_string(), new_block.desc().state_hash().to_string());
        log::info!("transactions_hash old:{} new:{}", block.desc().transactions_hash().to_string(), new_block.desc().transactions_hash().to_string());
        log::info!("receipts_hash old:{} new:{}", block.desc().receipts_hash().to_string(), new_block.desc().receipts_hash().to_string());

        if new_block.desc().calculate_id() == block.desc().calculate_id() {
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

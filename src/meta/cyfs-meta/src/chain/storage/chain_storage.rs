use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};
use cyfs_base::*;
use cyfs_base_meta::*;
use crate::state_storage::*;
use crate::executor::*;
use crate::executor::context::Config;
use std::str::FromStr;
use crate::chain::{BlockHeaderStorage, BlockStorage, TxStorage, to_meta_data};
use crate::get_meta_err_code;
use crate::stat::Stat;

const CURRENT_STORAGE_VER: u16 = 1;

pub type ChainStorageRef = Arc<ChainStorage>;
pub type ChainStorageWeakRef = Weak<ChainStorage>;

pub struct ChainStorage {
    block_storage: BlockStorage,
    header_storage: BlockHeaderStorage,
    tx_storage: TxStorage,
    state_storage: StorageRef,
}

impl ChainStorage {
    async fn init_components(dir: &Path) -> BuckyResult<(BlockStorage, BlockHeaderStorage, TxStorage)> {
        Ok((BlockStorage::new(dir.join("block"))?,
            BlockHeaderStorage::new(dir.join("db")).await?,
            TxStorage::new(dir.join("db")).await?,
            ))
    }

    pub fn state_storage(&self) -> &StorageRef {
        return &self.state_storage
    }

    pub async fn load(dir: &Path, new_storage: fn (path: &Path) -> StorageRef) -> BuckyResult<ChainStorageRef> {
        let (block_storage, header_storage, tx_storage) = Self::init_components(dir).await?;
        let ret = header_storage.load_tip_header().await;
        let state_storage = new_storage(dir.join("state_db").as_path());
        if ret.is_ok() {
            let tip_header = ret.unwrap();
            for i in 0..5 {
                if header_storage.backup_exist(tip_header.number() - i) && state_storage.backup_exist(tip_header.number() - i) {
                    header_storage.recovery(tip_header.number() - i)?;
                    state_storage.recovery(tip_header.number() - i).await?;
                    break;
                }
            }
        }

        {
            let state = state_storage.create_state(false).await;
            state.init_genesis(&Vec::new()).await?;
            state.init().await?;
            state.create_cycle_event_table(Config::new(&state)?.get_rent_cycle()).await?;
        }

        let state_hash = state_storage.state_hash().await.unwrap();
        log::info!("load state_hash:{} db:{}", state_hash.to_string(), state_storage.path().to_str().unwrap());

        Ok(Arc::new(Self {
            block_storage,
            header_storage,
            tx_storage,
            state_storage,
        }))
    }

    pub async fn reset(dir: PathBuf, block: Option<Block>, state_storage: StorageRef) -> BuckyResult<ChainStorageRef> {
        // assert_eq!(block.header().number(), 0);
        let (block_storage, header_storage, tx_storage) = Self::init_components(dir.as_path()).await?;
        if block.is_some() {
            block_storage.save_block(block.as_ref().unwrap())?;
            header_storage.save_genesis(block.as_ref().unwrap().header()).await?;
            state_storage.backup(block.unwrap().desc().number()).await?;
        }

        // storage_manager.save_snapshot(&block.header().hash(), storage)?;
        Ok(Arc::new(Self {
            header_storage,
            block_storage,
            tx_storage,
            state_storage,
        }))
    }

    pub async fn get_tip_info(&self) -> BuckyResult<(BlockDesc, StorageRef)> {
        let tip_header = self.header_storage.load_tip_header().await?;
        let tip_storage = self.state_storage.clone();
        Ok((tip_header, tip_storage))
    }

    pub async fn block_header(&self, block: ViewBlockEnum) -> BuckyResult<BlockDesc> {
        match block {
            ViewBlockEnum::Tip => {
                self.header_storage.load_tip_header().await
            },
            ViewBlockEnum::Number(n) => {
                self.header_storage.load_header_by_number(n).await
            },
            ViewBlockEnum::Hash(hash) => {
                let (header, _state) = self.header_storage.load_header_by_hash(&hash).await?;
                Ok(header)
            }
        }
    }

    // thread safe
    pub async fn add_mined_block(&self, block: &Block) -> BuckyResult<()> {
        self.block_storage.save_block(block)?;
        self.header_storage.save_header(block.header()).await?;
        self.header_storage.change_tip(block.header()).await?;
        self.tx_storage.add_block(block).await?;

        Ok(())
    }

    // thread safe
    pub async fn view(&self, request: ViewRequest, stat: Option<Stat>) -> BuckyResult<ViewResponse> {
        let header = self.block_header(request.block).await?;
        let storage = self.state_storage.clone();
        self.execute_view_request(&header, &storage.create_state(true).await, request.method, stat).await
    }

    async fn execute_view_request(&self, block: &BlockDesc, ref_state: &StateRef, enum_method: ViewMethodEnum, stat: Option<Stat>) -> BuckyResult<ViewResponse> {
        match enum_method {
            ViewMethodEnum::ViewBalance(method) => {
                Ok(ViewResponse::ViewBalance(ViewMethodExecutor::new(block, ref_state, stat, method).exec().await?))
            },
            ViewMethodEnum::ViewName(method) => {
                Ok(ViewResponse::ViewName(ViewMethodExecutor::new(block, ref_state, stat, method).exec().await?))
            },
            ViewMethodEnum::ViewDesc(method) => {
                Ok(ViewResponse::ViewDesc(ViewMethodExecutor::new(block, ref_state, stat, method).exec().await?))
            },
            ViewMethodEnum::ViewRaw(method) => {
                Ok(ViewResponse::ViewRaw(ViewMethodExecutor::new(block, ref_state, stat, method).exec().await?))
            },
            ViewMethodEnum::ViewStatus => {
                Ok(ViewResponse::ViewStatus(self.get_status().await?))
            },
            ViewMethodEnum::ViewBlock => {
                Ok(ViewResponse::ViewBlock(self.block_storage.load_block(&block.hash()).await?))
            },
            ViewMethodEnum::ViewTx(tx_id) => {
                Ok(ViewResponse::ViewTx(self.get_tx_full_info(&tx_id).await?))
            }
            ViewMethodEnum::ViewContract(method) => {
                Ok(ViewResponse::ViewContract(ViewMethodExecutor::new(block, ref_state, stat, method).exec().await?))
            }
            ViewMethodEnum::ViewBenifi(method) => {
                Ok(ViewResponse::ViewBenefi(ViewMethodExecutor::new(block,ref_state, stat, method).exec().await?))
            }
            ViewMethodEnum::ViewLog(method) => {
                Ok(ViewResponse::ViewLog(ViewMethodExecutor::new(block,ref_state, stat, method).exec().await?))
            }
            ViewMethodEnum::ViewNFT(nft_id) => {
                let (desc, name, state) = ref_state.nft_get(&nft_id).await?;
                let beneficiary = ref_state.get_beneficiary(&nft_id).await?;
                Ok(ViewResponse::ViewNFT((desc, name, beneficiary, state)))
            }
            ViewMethodEnum::ViewNFTApplyBuyList((nft_id, offset, length)) => {
                let sum = ref_state.nft_get_apply_buy_count(&nft_id).await?;
                let list = ref_state.nft_get_apply_buy_list(&nft_id, offset as i64, length as i64).await?;
                let mut ret_list = Vec::new();
                for (buyer_id, price, coin_id) in list.into_iter() {
                    ret_list.push(NFTBuyItem {
                        buyer_id,
                        price,
                        coin_id
                    })
                }
                Ok(ViewResponse::ViewNFTApplyBuyList(ViewNFTBuyListResult {
                    sum: sum as u32,
                    list: ret_list
                }))
            }
            ViewMethodEnum::ViewNFTBidList((nft_id, offset, length)) => {
                let sum = ref_state.nft_get_bid_count(&nft_id).await?;
                let list = ref_state.nft_get_bid_list(&nft_id, offset as i64, length as i64).await?;
                let mut ret_list = Vec::new();
                for (buyer_id, price, coin_id) in list.into_iter() {
                    ret_list.push(NFTBuyItem {
                        buyer_id,
                        price,
                        coin_id
                    })
                }
                Ok(ViewResponse::ViewNFTBidList(ViewNFTBuyListResult {
                    sum: sum as u32,
                    list: ret_list
                }))
            }
            ViewMethodEnum::ViewNFTLargestBuyValue(nft_id) => {
                match ref_state.nft_get(&nft_id).await {
                    Ok((_, _, state)) => {
                        if let NFTState::Auctioning(_) = state {
                            let list = ref_state.nft_get_bid_list(&nft_id, 0, i64::MAX).await?;
                            let mut ret = None;
                            for (buyer_id, price, coin_id) in list.into_iter() {
                                if ret.is_none() {
                                    ret = Some((buyer_id, coin_id, price));
                                } else {
                                    if ret.as_ref().unwrap().2 < price {
                                        ret = Some((buyer_id, coin_id, price));
                                    }
                                }
                            }
                            Ok(ViewResponse::ViewNFTLargestBuyValue(ret))
                        } else if NFTState::Normal == state {
                            let list = ref_state.nft_get_apply_buy_list(&nft_id, 0, i64::MAX).await?;
                            let mut ret = None;
                            for (buyer_id, price, coin_id) in list.into_iter() {
                                if ret.is_none() {
                                    ret = Some((buyer_id, coin_id, price));
                                } else {
                                    if ret.as_ref().unwrap().2 < price {
                                        ret = Some((buyer_id, coin_id, price));
                                    }
                                }
                            }
                            Ok(ViewResponse::ViewNFTLargestBuyValue(ret))
                        } else {
                            Ok(ViewResponse::ViewNFTLargestBuyValue(None))
                        }
                    },
                    Err(e) => {
                        if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                            let list = ref_state.nft_get_apply_buy_list(&nft_id, 0, i64::MAX).await?;
                            let mut ret = None;
                            for (buyer_id, price, coin_id) in list.into_iter() {
                                if ret.is_none() {
                                    ret = Some((buyer_id, coin_id, price));
                                } else {
                                    if ret.as_ref().unwrap().2 < price {
                                        ret = Some((buyer_id, coin_id, price));
                                    }
                                }
                            }
                            Ok(ViewResponse::ViewNFTLargestBuyValue(ret))
                        } else {
                            Err(e)
                        }
                    }
                }
            }
        }
    }

    // thread safe
    pub async fn receipt_of(&self, tx_hash: &TxHash) -> BuckyResult<Option<(Receipt, i64)>> {
        let (number, _) = self.tx_storage.get_tx_seq(tx_hash).await?;
        let header = self.header_storage.load_header_by_number(number).await?;
        let block = self.block_storage.load_block(&header.hash()).await?;
        let txes = block.transactions();
        for i in 0..txes.len() {
            if txes[i].desc().calculate_id().eq(tx_hash) {
                return Ok(Some((block.receipts()[i].clone(), number)));
            }
        }
        Ok(None)
    }

    pub async fn get_balance(&self, address_list: Vec<(u8, String)>) -> BuckyResult<Vec<String>> {
        let storage = self.state_storage.clone();
        let state = storage.create_state(true).await;

        let mut balance_list = Vec::new();
        for (coin_id, address) in address_list {
            let ctid = CoinTokenId::Coin(coin_id);
            let address_id = ObjectId::from_str(address.as_str())?;
            let balance = state.get_balance(&address_id, &ctid).await?;
            balance_list.push(balance.to_string());
        }
        Ok(balance_list)
    }

    pub async fn get_status(&self) -> BuckyResult<ChainStatus> {
        let block_header = self.header_storage.load_tip_header().await?;
        Ok(ChainStatus {
            version: 0,
            height: block_header.number(),
            gas_price: GasPrice {
                low: 0,
                medium: 0,
                high: 0
            }
        })
    }

    pub async fn get_tx_info(&self, tx_hash: &TxHash) -> BuckyResult<TxInfo> {
        let (number, index) = self.tx_storage.get_tx_seq(tx_hash).await?;
        let header = self.header_storage.load_header_by_number(number).await?;
        let (tx, receipt) = self.block_storage.get_tx_from_block(&header.hash(), index).await?;

        Ok(TxInfo {
            status: TX_STATUS_BLOCKED,
            tx: to_meta_data(&tx, &receipt)?,
            block_number: Some(number.to_string()),
            block_hash: Some(header.hash().to_string()),
            block_create_time: Some(bucky_time_to_js_time(header.create_time()).to_string())
        })
    }

    pub async fn get_tx_full_info(&self, tx_hash: &TxHash) -> BuckyResult<TxFullInfo> {
        let (number, index) = self.tx_storage.get_tx_seq(tx_hash).await?;
        let header = self.header_storage.load_header_by_number(number).await?;
        let (tx, receipt) = self.block_storage.get_tx_from_block(&header.hash(), index).await?;
        Ok(TxFullInfo {
            status: TX_STATUS_BLOCKED,
            block_number: number,
            tx,
            receipt: Some(receipt)
        })
    }

    pub async fn get_block_info_by_number(&self, number: i64) -> BuckyResult<BlockInfo> {
        let header = self.header_storage.load_header_by_number(number).await?;
        let block = self.block_storage.load_block(&header.hash()).await?;
        let mut meta_tx_list = Vec::new();
        let tx_list = block.transactions();
        let receipt_list = block.receipts();
        let mut index = 0;
        for tx in tx_list {
            let receipt= receipt_list.get(index).unwrap();
            meta_tx_list.push(TxInfo {
                status: 0,
                tx: to_meta_data(tx, receipt)?,
                block_number: Some(block.desc().number().to_string()),
                block_hash: Some(block.desc().hash().to_string()),
                block_create_time: Some(bucky_time_to_js_time(block.desc().create_time()).to_string()),
            }
            );
            index += 1;
        }
        Ok(BlockInfo {
            height: number,
            block_hash: header.hash().to_string(),
            create_time: bucky_time_to_js_time(header.create_time()),
            tx_list: meta_tx_list
        })
    }

    pub async fn get_blocks_info_by_range(&self, start_block: i64, end_block: i64) -> BuckyResult<Vec<BlockInfo>> {
        let tip_header = self.header_storage.load_tip_header().await?;
        let height = tip_header.number();

        let mut last_block = height;
        if end_block != -1 && end_block < height {
            last_block = end_block;
        }
        let mut block_list = Vec::new();

        for i in start_block..last_block+1 {
            let block = self.get_block_info_by_number(i).await?;
            block_list.push(block);
        }

        Ok(block_list)
    }

    pub async fn get_block_info_by_hash(&self, hash: &str) -> BuckyResult<BlockInfo> {
        let block = self.block_storage.load_block(&BlockHash::clone_from_hex(hash, &mut Vec::new())?).await?;
        let mut meta_tx_list = Vec::new();
        let tx_list = block.transactions();
        let receipt_list = block.receipts();
        let mut index = 0;
        for tx in tx_list {
            let receipt= receipt_list.get(index).unwrap();
            meta_tx_list.push(TxInfo {
                status: 0,
                tx: to_meta_data(tx, receipt)?,
                block_number: Some(block.desc().number().to_string()),
                block_hash: Some(hash.to_string()),
                block_create_time: Some(bucky_time_to_js_time(block.desc().create_time()).to_string()),
            }
            );
            index += 1;
        }
        Ok(BlockInfo {
            height: block.header().number(),
            block_hash: block.header().hash().to_string(),
            create_time: block.header().create_time(),
            tx_list: meta_tx_list
        })
    }

    pub async fn get_block_by_number(&self, number: i64) -> BuckyResult<Block> {
        let header = self.header_storage.load_header_by_number(number).await?;
        self.block_storage.load_block(&header.hash()).await
    }

    pub async fn backup(&self, height: i64) -> BuckyResult<()> {
        self.header_storage.backup(height)?;
        self.state_storage.backup(height).await
    }

    pub async fn recovery(&self, height: i64) -> BuckyResult<()> {
        self.header_storage.recovery(height)?;
        self.state_storage.recovery(height).await
    }
}

#[cfg(test)]
pub mod chain_storage_tests {
    use std::convert::TryFrom;
    use crate::chain::{ChainStorage};
    use std::fs::{remove_dir_all, create_dir};
    use cyfs_base_meta::*;
    use cyfs_base::*;
    use crate::{new_sql_storage, BlockBody, State, new_archive_storage, Archive};
    use crate::chain::chain_storage::ChainStorageRef;

    pub fn create_people() -> StandardObject {
        let private_key = PrivateKey::generate_rsa(1024).unwrap();
        let public_key = private_key.public();
        StandardObject::Device(Device::new(None
                                           , UniqueId::default()
                                           , Vec::new()
                                           , Vec::new()
                                           , Vec::new()
                                           , public_key
                                           , Area::default()
                                           , DeviceCategory::OOD).build())
    }

    pub fn create_test_tx(people: &StandardObject, nonce: i64, to: &StandardObject, value: i64) -> MetaTx {
        let body = MetaTxBody::TransBalance(TransBalanceTx {
            ctid: CoinTokenId::Coin(0),
            to: vec![(to.calculate_id(), value)]
        });
        let tx = MetaTx::new(nonce, TxCaller::try_from(people).unwrap()
                             , 0
                             , 0
                             , 0
                             , None
                             , body, Vec::new()).build();
        tx
    }

    pub async fn create_test_chain_storage(storage_name: &str) -> ChainStorageRef {
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
        temp_dir.push(storage_name);
        println!("{}", temp_dir.to_str().unwrap());
        if temp_dir.exists() {
            remove_dir_all(temp_dir.clone()).unwrap();
        }
        create_dir(temp_dir.clone()).unwrap();

        let mut genesis_storage = temp_dir.clone();
        genesis_storage.push("state_db");
        let storage = new_sql_storage(genesis_storage.as_path());
        {
            let state = storage.create_state(false).await;
            state.init_genesis(&config.coins).await.unwrap();
        }


        let mut archive_path = temp_dir.clone();
        archive_path.push("test3");
        let archive_storage = new_archive_storage(archive_path.as_path(), true);
        {
            let archive = archive_storage.create_archive(false).await;
            archive.init().await.unwrap();
        }

        let block = Block::new(ObjectId::default(), None, HashValue::default(), BlockBody::new()).unwrap().build();
        ChainStorage::reset(temp_dir, Some(block), storage).await.unwrap()
    }

    #[test]
    fn test() {
        async_std::task::block_on(async {
            let storage = create_test_chain_storage("test3").await;
            let people1 = create_people();
            let people2 = create_people();

            let tip = storage.block_header(ViewBlockEnum::Tip).await.unwrap();

            // let mut header = BlockDesc::new(BlockDescContent::new(ObjectId::default(), Some(&tip))).build();

            let mut block_body = BlockBody::new();
            let tx = create_test_tx(&people1, 1, &people2, 10);
            let tx_id = tx.desc().calculate_id();
            block_body.add_transaction(tx).unwrap();
            block_body.add_receipts(vec![Receipt::new(0,0)]).unwrap();

            let block = Block::new(ObjectId::default(), Some(&tip), HashValue::default(), block_body).unwrap().build();
            let ret = storage.add_mined_block(&block).await;
            assert!(ret.is_ok());

            let ret = storage.get_tx_info(&tx_id).await;
            assert!(ret.is_ok());
            assert_eq!(ret.as_ref().unwrap().status, TX_STATUS_BLOCKED);
            assert_eq!(ret.as_ref().unwrap().block_number.as_ref().unwrap(), "1");

            let ret = storage.get_status().await;
            assert!(ret.is_ok());
            assert_eq!(ret.as_ref().unwrap().height, 1);
        });
    }
}

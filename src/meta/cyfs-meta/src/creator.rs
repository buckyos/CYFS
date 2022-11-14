use std::path::{Path, PathBuf};
use std::fs::File;
use serde_json;
use serde::{Serialize, Deserialize};
use cyfs_base::*;
use cyfs_base_meta::*;
use crate::state_storage::*;
use crate::chain::*;
use log::*;
use crate::executor::context::Config;
use crate::mint::btc_mint::BTCMint;
use crate::network::{HttpTcpChainNetwork};
use async_std::sync::Arc;
use crate::http_get_request;
use crate::mint::subchain_mint::SubChainMint;
use crate::*;
use std::sync::Mutex;

pub struct ChainCreator {
    reserved: (std::marker::PhantomData<StorageRef>, std::marker::PhantomData<dyn State>)
}

#[derive(Serialize, Deserialize)]
pub struct MinerConfig {
    pub coinbase: ObjectId,
    pub interval: u32,
    pub bfc_spv_node: String,
    pub chain_type: Option<String>,
    pub bft_port: Option<u16>,
    pub bft_node_list: Option<Vec<String>>,
    pub miner_key_path: Option<String>,
}

lazy_static::lazy_static! {
    pub static ref MINER: Mutex<Option<MinerRef>> = Mutex::new(None);
}

impl ChainCreator {
    pub fn create_chain(config_path: &Path, output_path: &Path, new_storage: fn (path: &Path) -> StorageRef, trace: bool, new_archive_storage: fn (path: &Path, trace: bool) -> ArchiveStorageRef) -> BuckyResult<MinerRef> {
        async_std::task::block_on(async move {
            let config_file = File::open(config_path).map_err(|err| {
                error!("open file {} failed, err {}", config_path.display(), err);
                crate::meta_err!(ERROR_NOT_FOUND)
            })?;
            let config: GenesisConfig = serde_json::from_reader(config_file).map_err(|err| {
                error!("invalid config.json, err {}", err);
                crate::meta_err!(ERROR_PARAM_ERROR)
            })?;

            let root_path = config_path.parent().unwrap();

            let storage = new_storage(output_path.join("state_db").as_path());
            let archive_storage = new_archive_storage(output_path.join("archive_db").as_path(), trace);
            let header = BlockDesc::new(BlockDescContent::new(config.coinbase, None)).build();
            let mut block_body = BlockBody::new();
            {
	            let state = storage.create_state(false).await;
	            state.init_genesis(&config.coins).await?;
	            state.init().await?;
	            let meta_config = Config::new(&state)?;
	            state.create_cycle_event_table(meta_config.get_rent_cycle()).await?;
	            let state_hash = storage.state_hash().await?;

	            log::info!("create state_hash:{}", state_hash.to_string());

                let archive = archive_storage.create_archive(false).await;

	            if config.chain_type.is_none() || config.chain_type.as_ref().unwrap() == "standalone" {
                    let btc_mint = BTCMint::new(&state, &meta_config, config.bfc_spv_node.as_str());
	                if let Ok(coinage_tx) = btc_mint.create_btc_genesis_tx() {
	                    let tx = MetaTx::new(1, TxCaller::Miner(ObjectId::default()), 0, 0, 0
	                                     , None, MetaTxBody::BTCCoinageRecord(coinage_tx), Vec::new()).build();
	                    block_body.add_transaction(tx).unwrap();
	                }
	            }
	            else if config.chain_type.as_ref().unwrap() == "bft" {
	                let miner_key_path = {
	                    let path = PathBuf::from(config.miner_key_path.as_ref().unwrap());
	                    if path.is_absolute() {
	                        path
	                    } else {
	                        let mut root_path = PathBuf::from(root_path);
	                        root_path.push(path);
	                        root_path
	                    }
	                };

	                let miner_group_path = {
	                    let path = PathBuf::from(config.mg_path.as_ref().unwrap());
	                    if path.is_absolute() {
	                        path
	                    } else {
	                        let mut root_path = PathBuf::from(root_path);
	                        root_path.push(path);
	                        root_path
	                    }
	                };

	                let miner_desc_path = {
	                    let path = PathBuf::from(config.miner_desc_path.as_ref().unwrap());
	                    if path.is_absolute() {
	                        path
	                    } else {
	                        let mut root_path = PathBuf::from(root_path);
	                        root_path.push(path);
	                        root_path
	                    }
	                };

	                let (miner_key, _) = PrivateKey::decode_from_file(
	                    miner_key_path.as_path(), &mut Vec::new())?;
	                let (miner_group, _) = MinerGroup::decode_from_file(
	                    miner_group_path.as_path(), &mut Vec::new())?;
	                let (desc, _) = Device::decode_from_file(
	                    miner_desc_path.as_path(), &mut Vec::new())?;

	                let meta_create_tx = MetaTxBody::CreateMinerGroup(miner_group);
	                let mut tx = MetaTx::new(1,
	                                     TxCaller::Device(desc.desc().clone()),
	                                     0,
	                                     0,
	                                     0,
	                                     None,
	                                         meta_create_tx,
	                                     Vec::new()).build();
	                tx.sign(miner_key.clone())?;
	                block_body.add_transaction(tx).unwrap();

                    let btc_mint = BTCMint::new(&state, &meta_config, config.bfc_spv_node.as_str());
	                if let Ok(coinage_tx) = btc_mint.create_btc_genesis_tx() {
	                    let mut tx = MetaTx::new(2,
	                                         TxCaller::Device(desc.desc().clone()),
	                                         0,
	                                         0,
	                                         0,
	                                         None,
	                                             MetaTxBody::BTCCoinageRecord(coinage_tx),
	                                         Vec::new()).build();
	                    tx.sign(miner_key)?;
	                    block_body.add_transaction(tx).unwrap();
	                }
	            } else if config.chain_type.as_ref().unwrap() == "bft_sub" {
                    let url = format!("{}/tx_full/{}", config.bfc_spv_node, config.sub_chain_tx.unwrap());
                    let tx_full_info = Result::<TxFullInfo, u32>::clone_from_slice(http_get_request(url.as_str()).await?.as_slice())?;
                    if tx_full_info.is_err() {
                        log::error!("get creat tx info failed");
                        return Err(meta_err!(*tx_full_info.as_ref().err().unwrap()))
                    }

                    let tx_full_info = tx_full_info.unwrap();
                    if let MetaTxBody::CreateSubChainAccount(miner_group) = &tx_full_info.tx.desc().content().body.get_obj()[0] {
                        let miner_key_path = {
                            let path = PathBuf::from(config.miner_key_path.as_ref().unwrap());
                            if path.is_absolute() {
                                path
                            } else {
                                let mut root_path = PathBuf::from(root_path);
                                root_path.push(path);
                                root_path
                            }
                        };

                        let miner_desc_path = {
                            let path = PathBuf::from(config.miner_desc_path.as_ref().unwrap());
                            if path.is_absolute() {
                                path
                            } else {
                                let mut root_path = PathBuf::from(root_path);
                                root_path.push(path);
                                root_path
                            }
                        };

                        let (miner_key, _) = PrivateKey::decode_from_file(
                            miner_key_path.as_path(), &mut Vec::new())?;
                        let (desc, _) = Device::decode_from_file(
                            miner_desc_path.as_path(), &mut Vec::new())?;

                        let meta_create_tx = MetaTxBody::CreateMinerGroup(miner_group.clone());
                        let mut tx = MetaTx::new(1,
                                                 TxCaller::Device(desc.desc().clone()),
                                                 0,
                                                 0,
                                                 0,
                                                 None,
                                                 meta_create_tx,
                                                 Vec::new()).build();
                        tx.sign(miner_key.clone())?;
                        block_body.add_transaction(tx).unwrap();

                        let sub_chain_mint = SubChainMint::new(miner_group.desc().calculate_id(),
                                                               &state, &meta_config, config.bfc_spv_node.clone());
                        if let Ok(coinage_tx) = sub_chain_mint.create_genesis_tx().await {
                            let mut tx = MetaTx::new(2,
                                                     TxCaller::Device(desc.desc().clone()),
                                                     0,
                                                     0,
                                                     0,
                                                     None,
                                                     MetaTxBody::SubChainCoinageRecord(coinage_tx),
                                                     Vec::new()).build();
                            tx.sign(miner_key)?;
                            block_body.add_transaction(tx).unwrap();
                        }
                    } else {
                        log::error!("tx type error");
                        return Err(meta_err!(ERROR_EXCEPTION));
                    }
	            }

	            // state.being_transaction().await?;
                let ret = BlockExecutor::execute_block(&header,
                                                       &mut block_body,
                                                       &state,
                                                       &archive,
                                                       &meta_config,
                                                       None,
                                                       config.bfc_spv_node.clone(),
                                                       None,
                                                       ObjectId::default()).await;
                if ret.is_ok() {
	                // state.commit().await?;
	            } else {
	                // state.rollback().await?;
	                ret?;
	            }
			}
            let state_hash = storage.state_hash().await?;
            log::info!("create state_hash2:{}", state_hash.to_string());

            let mut block = Block::new(config.coinbase, None, state_hash, block_body)?.build();

            let ret: BuckyResult<MinerRef> = if config.chain_type.is_none() || config.chain_type.as_ref().unwrap() == "standalone" {
                let chain = Chain::new(output_path.to_path_buf(), Some(block), storage, archive_storage).await?;
                let miner = StandaloneMiner::new(
                    config.coinbase.clone(),
                    config.interval,
                    chain,
                    config.bfc_spv_node.clone())?;
                Ok(Arc::new(miner))
            } else if config.chain_type.as_ref().unwrap() == "bft" || config.chain_type.as_ref().unwrap() == "bft_sub" {
                let miner_key_path = {
                    let path = PathBuf::from(config.miner_key_path.unwrap());
                    if path.is_absolute() {
                        path
                    } else {
                        let mut root_path = PathBuf::from(root_path);
                        root_path.push(path);
                        root_path
                    }
                };
                let (miner_key, _) = PrivateKey::decode_from_file(
                    miner_key_path.as_path(), &mut Vec::new())?;
                let public_key = miner_key.public();
                block.sign(miner_key.clone(), &SignatureSource::Key(PublicKeyValue::Single(public_key))).await?;
                let chain = Chain::new(output_path.to_path_buf(), Some(block), storage, archive_storage).await?;
                let miner = BFTMiner::new(
                    "bft".to_owned(),
                    config.coinbase.clone(),
                    config.interval,
                    chain,
                    config.bfc_spv_node.clone(),
                    HttpTcpChainNetwork::new(0, Vec::new()),
                    miner_key)?;
                Ok(Arc::new(miner))
            } else {
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, "InvalidParam"))
            };
            ret
        })
    }

    pub fn start_miner_instance(dir: &Path, new_storage: fn (path: &Path) -> StorageRef, trace: bool, new_archive_storage: fn (path: &Path, trace: bool) -> ArchiveStorageRef) -> BuckyResult<Arc<dyn Miner>> {
        async_std::task::block_on(async move {
            let config_file = File::open(dir.join("config.json")).map_err(|err| {
                error!("open config.json at {} failed, err {}", dir.display(), err);
                crate::meta_err!(ERROR_NOT_FOUND)})?;
            let config: MinerConfig = serde_json::from_reader(config_file).map_err(|err| {
                error!("invalid config.json, err {}", err);
                crate::meta_err!(ERROR_PARAM_ERROR)})?;
            let ret: BuckyResult<Arc<dyn Miner>> = if config.chain_type.is_none() || config.chain_type.as_ref().unwrap() == "standalone" {
                let miner = StandaloneMiner::load(
                    config.coinbase.clone(), 
                    config.interval, 
                    config.bfc_spv_node.clone(),
                    dir,
                    new_storage,
                    trace,
                    new_archive_storage).await?;
                let miner_ref = Arc::new(miner);
                let mut miner_lock = MINER.lock().unwrap();
                *miner_lock = Some(miner_ref.clone());

                miner_ref.run()?;
                // let miner_ref: MinerRef = miner_ref;
                Ok(miner_ref)
            } else if config.chain_type.as_ref().unwrap() == "bft" || config.chain_type.as_ref().unwrap() == "bft_sub" {
                let miner_key_path = {
                    let path = PathBuf::from(config.miner_key_path.unwrap());
                    if path.is_absolute() {
                        path
                    } else {
                        let mut root_path = PathBuf::from(dir);
                        root_path.push(path);
                        root_path
                    }
                };
                let (miner_key, _) = PrivateKey::decode_from_file(
                    miner_key_path.as_path(), &mut Vec::new())?;
                let mut node_list = Vec::new();
                for node in &config.bft_node_list.unwrap() {
                    node_list.push(("unknown".to_owned(), node.clone()))
                }
                let network = HttpTcpChainNetwork::new(config.bft_port.unwrap(), node_list);
                let miner = BFTMiner::load(config.chain_type.as_ref().unwrap().to_owned(),
                                           config.coinbase.clone(),
                                           config.interval,
                                           config.bfc_spv_node.clone(),
                                           dir,
                                           new_storage,
                                           trace,
                                           new_archive_storage,
                                           network,
                                           miner_key).await?;
                let miner_ref = Arc::new(miner);
                let mut miner_lock = MINER.lock().unwrap();
                *miner_lock = Some(miner_ref.clone());
                miner_ref.run()?;
                log::info!("miner {} startup", config.coinbase.to_string());
                Ok(miner_ref)
            } else {
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, "InvalidParam"))
            };
            ret
        })
        // let chain = Chain::load(dir, new_storage)?;
        // let miner = StandaloneMiner::new(config.coinbase.clone(), config.interval
        //                             , chain, config.bfc_spv_node.clone())?;
        // Ok(Box::new(miner))
    }

    pub fn start_chain_instance(_dir: &Path, _new_storage: fn (path: &Path) -> StorageRef) -> BuckyResult<ChainStorage> {
        unimplemented!()
    }
}




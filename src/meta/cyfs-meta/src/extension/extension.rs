use cyfs_base::BuckyResult;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use async_trait::async_trait;
use crate::executor::context;
use crate::{ExecuteContext, StateRef, EventResult, TxLog};
use cyfs_base_meta::{MetaExtensionType, MetaTx, BlockDesc, ExtensionEvent};
use lazy_static::lazy_static;

#[async_trait]
pub trait MetaExtension: Send + Sync {
    fn get_extension_id(&self) -> MetaExtensionType;
    async fn init(&self, state_ref: &StateRef) -> BuckyResult<()>;
    async fn execute_tx(&self, context: &mut ExecuteContext, fee_counter: &mut context::FeeCounter, tx: &MetaTx, data: &Vec<u8>) -> BuckyResult<Vec<TxLog>>;
    async fn on_event(&self, state_ref: &StateRef, block_desc: &BlockDesc, event: &ExtensionEvent) -> BuckyResult<EventResult>;
}
pub type MetaExtensionRef = Arc<dyn MetaExtension>;

pub struct MetaExtensionManager {
    extension_map: HashMap<u32, MetaExtensionRef>
}

lazy_static! {
    static ref MANAGER: Mutex<MetaExtensionManager> = Mutex::new(MetaExtensionManager {
                extension_map: HashMap::new()
            });
}

impl MetaExtensionManager {
    pub fn register_extension(extension: MetaExtensionRef) {
        let mut manager = MANAGER.lock().unwrap();
        manager.extension_map.insert(extension.get_extension_id() as u32, extension);
    }

    pub fn get_extension(extension_id: &MetaExtensionType) -> Option<MetaExtensionRef> {
        let manager = MANAGER.lock().unwrap();
        let extension = manager.extension_map.get(&(*extension_id as u32));
        if extension.is_some() {
            Some(extension.unwrap().clone())
        } else {
            None
        }
    }

    pub async fn init_extension(state_ref: &StateRef) -> BuckyResult<()> {
        let extension_map = {
            let manager = MANAGER.lock().unwrap();
            manager.extension_map.clone()
        };
        for (_, extension) in extension_map {
            extension.init(state_ref).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test_extension {
    use crate::*;
    use cyfs_base_meta::*;
    use cyfs_base::*;
    use crate::executor::context::{FeeCounter, Config, UnionWithdrawManager};
    use crate::events::event_manager::EventManager;
    use crate::rent::rent_manager::RentManager;
    use crate::name_auction::auction::Auction;
    use crate::executor::tx_executor::TxExecutor;
    use std::str::FromStr;
    use std::convert::TryFrom;
    use async_trait::async_trait;
    use std::sync::Arc;

    #[derive(RawEncode, RawDecode, Clone)]
    struct TestTx {
        i: u32,
    }

    pub struct TestMetaExtension {

    }

    #[async_trait]
    impl MetaExtension for TestMetaExtension {
        fn get_extension_id(&self) -> MetaExtensionType {
            MetaExtensionType::DSG
        }

        async fn init(&self, state_ref: &StateRef) -> BuckyResult<()> {
            unimplemented!()
        }

        async fn execute_tx(&self, _context: &mut ExecuteContext, _fee_counter: &mut FeeCounter, tx: &MetaTx, data: &Vec<u8>) -> BuckyResult<Vec<TxLog>> {
            let ret = TestTx::clone_from_slice(data.as_slice());
            assert!(ret.is_ok());
            assert_eq!(ret.unwrap().i, 1);
            return Ok(Vec::new())
        }

        async fn on_event(&self, state_ref: &StateRef, block_desc: &BlockDesc, event: &ExtensionEvent) -> BuckyResult<EventResult> {
            unimplemented!()
        }
    }

    #[test]
    fn test() {
        async_std::task::block_on(async {
            MetaExtensionManager::register_extension(Arc::new(TestMetaExtension{}));

            let state = sql_storage_tests::create_state().await;
            let config = Config::new(&state).unwrap();
            let ret = state.create_cycle_event_table(config.get_rent_cycle()).await;
            assert!(ret.is_ok());

            let event_manager = EventManager::new(&state, &config);
            let rent_manager = RentManager::new(&state, &config, &event_manager);
            let auction = Auction::new(&state, &config, &rent_manager, &event_manager);
            let union_withdraw_manager = UnionWithdrawManager::new(&state, &config, &event_manager);
            let nft_auction = NFTAuction::new(&state, &config, &event_manager);
            let executor = TxExecutor::new(&state, &config, &rent_manager, &auction, &event_manager,
                                           &union_withdraw_manager, &nft_auction, "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true);

            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn6").unwrap();

            let private_key1 = PrivateKey::generate_rsa(1024).unwrap();
            let device1 = Device::new(
                None
                , UniqueId::default()
                , Vec::new()
                , Vec::new()
                , Vec::new()
                , private_key1.public()
                , Area::default()
                , DeviceCategory::OOD).build();
            let id1 = device1.desc().calculate_id();

            let private_key2 = PrivateKey::generate_rsa(1024).unwrap();
            let device2 = Device::new(
                None
                , UniqueId::default()
                , Vec::new()
                , Vec::new()
                , Vec::new()
                , private_key2.public()
                , Area::default()
                , DeviceCategory::OOD).build();
            let id2 = device2.desc().calculate_id();

            let mut nonce1 = 1;
            let ctid = CoinTokenId::Coin(0);
            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            let start_block = 2;
            for i in 1..10 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 1 {
                    state.inc_balance(&ctid, &id1, 300).await.unwrap();
                    state.inc_balance(&ctid, &id2, 300).await.unwrap();
                } else if i == start_block {
                    let test_tx = TestTx {
                        i: 1,
                    };
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::Extension(MetaExtensionTx {
                            extension_id: MetaExtensionType::DSG,
                            tx_data: test_tx.to_vec().unwrap()
                        })
                        , Vec::new()).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);
                }

                event_manager.run_event(&new).await.unwrap();
                prev = new;
            }

        })
    }
}

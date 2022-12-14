use crate::state_storage::{StateRef, StateWeakRef};
use crate::executor::context::{ConfigRef, ConfigWeakRef, UnionWithdrawManagerWeakRef, UnionWithdrawManagerRef};
use cyfs_base_meta::*;
use crate::executor::context;
use cyfs_base::*;
use crate::rent::rent_manager::{RentManagerRef, RentManagerWeakRef};
use crate::name_auction::auction::{AuctionRef, AuctionWeakRef};
use crate::executor::transaction::ExecuteContext;

use log::*;
use crate::helper::{ArcWeakHelper};
use crate::events::event_manager::{EventManagerWeakRef, EventManagerRef};
use std::i64::MAX;
use cyfs_base::TxCaller::Miner;
use cyfs_base::NamedObject;
use crate::mint::btc_mint::BTCMint;
use crate::{MetaExtensionManager, State};
use crate::*;
use crate::chain::chain_storage::{ChainStorageRef};
use crate::meta_backend::MetaBackend;
use crate::stat::Stat;

pub struct TxExecutor {
    pub ref_state: StateWeakRef,
    pub config: ConfigWeakRef,
    pub rent_manager: RentManagerWeakRef,
    pub auction: AuctionWeakRef,
    pub event_manager: EventManagerWeakRef,
    pub union_withdraw_manager: UnionWithdrawManagerWeakRef,
    pub nft_auction: NFTAuctionWeakRef,
    pub evm_config: evm::Config,
    pub mint_url: String,
    pub miner_key: Option<PrivateKey>,
    pub miner_id: ObjectId,
    pub is_verify_block: bool,
    pub stat: Option<Stat>
}

impl TxExecutor {
    pub fn new(state: &StateRef
               , config: &ConfigRef
               , rent_manager: &RentManagerRef
               , auction: &AuctionRef
               , event_manager: &EventManagerRef
               , union_withdraw_manager: &UnionWithdrawManagerRef
               , nft_auction: &NFTAuctionRef
               , mint_url: String
               , miner_key: Option<PrivateKey>
               , miner_id: ObjectId
               , is_verify_block: bool, stat: Option<Stat>) -> TxExecutor {
        let weak_ref_state = StateRef::downgrade(state);
        event_manager.register_listener(EventType::NFTCancelApplyBuy, move |_cur_block: BlockDesc, event: Event| {
            let weak_ref_state = weak_ref_state.clone();
            Box::pin(async move {
                if let Event::NFTCancelApplyBuy(event) = event {
                    log::info!("nft {} user {} cancel apply buy", event.nft_id.to_string(), event.user_id.to_string());
                    let ret = weak_ref_state.to_rc()?.nft_get_apply_buy(&event.nft_id, &event.user_id).await?;
                    if ret.is_some() {
                        let (price, coin_id) = ret.unwrap();
                        weak_ref_state.to_rc()?.inc_balance(&coin_id, &event.user_id, price as i64).await?;
                    }
                    Ok(EventResult::new(0, Vec::new()))
                } else {
                    Err(crate::meta_err!(ERROR_INVALID))
                }
            })
        });
        let weak_ref_state = StateRef::downgrade(state);
        event_manager.register_listener(EventType::NFTStopSell, move |_cur_block: BlockDesc, event: Event| {
            let weak_ref_state = weak_ref_state.clone();
            Box::pin(async move {
                if let Event::NFTStopSell(event) = event {
                    log::info!("nft {} stop sell", event.nft_id.to_string());
                    let (_, _, state) = weak_ref_state.to_rc()?.nft_get(&event.nft_id).await?;
                    if let NFTState::Selling(_) = state {
                        let new_state = NFTState::Normal;
                        weak_ref_state.to_rc()?.nft_update_state(&event.nft_id, &new_state).await?;
                    }
                    Ok(EventResult::new(0, Vec::new()))
                } else {
                    Err(meta_err!(ERROR_INVALID))
                }
            })
        });
        TxExecutor {
            ref_state: StateRef::downgrade(state),
            config: ConfigRef::downgrade(config),
            rent_manager: RentManagerRef::downgrade(rent_manager),
            auction: AuctionRef::downgrade(auction),
            event_manager: EventManagerRef::downgrade(event_manager),
            union_withdraw_manager: UnionWithdrawManagerRef::downgrade(union_withdraw_manager),
            nft_auction: NFTAuctionRef::downgrade(nft_auction),
            evm_config: evm::Config::istanbul(),    // 先把evm的config创建在这里，以后能自己设置了，应该是外边传进来的
            mint_url,
            miner_key,
            miner_id,
            is_verify_block,
            stat
        }
    }

    pub async fn execute(&self, owner_block: &BlockDesc, tx: &MetaTx, chain_storage: Option<&ChainStorageRef>) -> BuckyResult<Receipt> {
        let mut caller = context::Account::from_caller(&tx.desc().content().caller, &self.ref_state.to_rc()?)?;
        let caller_id = caller.id().clone();

        info!("miner execute block {} tx {:?} caller {} nonce {} thread {:?}",
              owner_block.number(),
              tx.desc().calculate_id().to_string(),
              caller_id.to_string(),
              tx.desc().content().nonce,
              std::thread::current().id());

        // verify nonce
        let nonce = caller.nonce().await?;
        if let Miner(_) = tx.desc().content().caller {

        } else {
            if nonce + 1 != tx.desc().content().nonce {
                error!("execute tx failed for invalid nonce expected {:?} but {:?} thread {:?}",
                       nonce + 1, tx.desc().content().nonce, std::thread::current().id());

                return Ok(Receipt::new(1,0));
                // return Err(crate::meta_err!(ERROR_INVALID));
            }
        }
        caller.inc_nonce().await?;

        let total_fee: i64 = tx.desc().content().max_fee as i64 * tx.desc().content().gas_price as i64;

        //TODO：测试链接账号自动充值
        for body in tx.desc().content().body.get_obj() {
            if let MetaTxBody::CreateDesc(_) = body {
                let body = tx.body();
                if body.is_none() {
                    continue;
                }
                let data = &body.as_ref().unwrap().content().data;
                let ret = SavedMetaObject::clone_from_slice(data.as_slice());
                if ret.is_err() {
                    continue;
                }

                let desc = ret.unwrap();
                if let SavedMetaObject::People(_) = &desc {
                    // 给0号主币充值
                    self.ref_state
                        .to_rc()?
                        .inc_balance(&CoinTokenId::Coin(0), caller.id(), 100000000)
                        .await?;
                } else if let SavedMetaObject::Device(_) = &desc {
                    self.ref_state
                        .to_rc()?
                        .inc_balance(&CoinTokenId::Coin(0), caller.id(), total_fee)
                        .await?;
                }
            }
        }


        // update balance = balance - max_fee
        self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(tx.desc().content().gas_coin_id), caller.id(), total_fee).await.map_err(|_| crate::meta_err!(ERROR_INVALID))?;

        let mut context = ExecuteContext::new(&self.ref_state.to_rc()?,
                                              owner_block,
                                              caller,
                                              &self.config.to_rc()?,
                                              &self.event_manager.to_rc()?,
                                              self.is_verify_block);

        let mut fee_counter = context::FeeCounter::new(tx.desc().content().max_fee);

        self.ref_state.to_rc()?.being_transaction().await?;
        //TODO: cost some fee for tx's storage
        let mut result = Ok(());
        let mut address = None;
        let mut return_value = None;
        let mut result_code: u16 = ERROR_SUCCESS;
        let mut logs = vec![];

        let mut backend = MetaBackend::new(
            &self.ref_state.to_rc()?,
            tx.desc().content().gas_price as u64,
            owner_block,
            caller_id,
            chain_storage,
            self.evm_config.clone()
        );

        for body in tx.desc().content().body.get_obj() {
            match body {
                MetaTxBody::TransBalance(ref tx) => {
                    let result2 = self.execute_trans_balance(&mut context, &mut fee_counter, tx, &mut backend, &self.evm_config).await;
                    if result2.is_err() {
                        result = Err(result2.err().unwrap());
                    } else {
                        logs.append(&mut result2.unwrap());
                    }
                },
                MetaTxBody::CreateUnion(ref _tx) => {
                    result = self.execute_create_union(&mut context, &mut fee_counter, _tx).await;
                }
                MetaTxBody::DeviateUnion(ref _tx) => {
                    result = self.execute_deviate_union(&mut context, &mut fee_counter, _tx).await;
                },
                MetaTxBody::WithdrawFromUnion(ref _tx) => {
                    result = self.execute_withdraw_from_union(&mut context, &mut fee_counter, _tx).await;
                },
                MetaTxBody::CreateDesc(ref _tx) => {
                    result = self.execute_trans_and_create_desc_tx(&mut context, &mut fee_counter, _tx, tx).await;
                },
                MetaTxBody::UpdateDesc(ref update_tx) => {
                    result = self.execute_update_desc_tx(&mut context, &mut fee_counter, update_tx, tx).await;
                },
                MetaTxBody::RemoveDesc(ref tx) => {
                    result = self.execute_remove_desc_tx(&mut context, &mut fee_counter, tx).await;
                },
                MetaTxBody::BidName(ref tx) => {
                    result = self.execute_bid_name_tx(&mut context, &mut fee_counter, tx).await;
                },
                MetaTxBody::UpdateName(ref tx) => {
                    result = self.execute_update_name_info_tx(&mut context, &mut fee_counter, tx).await;
                },
                MetaTxBody::AuctionName(ref tx) => {
                    result = self.auction.to_rc()?.active_auction_name(tx.name.as_str(), MAX, tx.price as i64).await;
                },
                MetaTxBody::CancelAuctionName(ref tx) => {
                    result = self.execute_cancel_auction_name_tx(&mut context, &mut fee_counter, tx).await;
                },
                MetaTxBody::BuyBackName(ref tx) => {
                    result = self.auction.to_rc()?.buy_back_name(owner_block, tx.name.as_str(), &caller_id).await;
                },
                MetaTxBody::SetConfig(ref tx) => {
                    result = self.set_config_tx(&mut context, &mut fee_counter, tx).await;
                },
                MetaTxBody::WithdrawToOwner(ref tx) => {
                    result = self.execute_withdraw_to_owner(&mut context, &mut fee_counter, tx).await;
                },
                MetaTxBody::BTCCoinageRecord(ref tx) => {
                    let btc_mint = BTCMint::new(&self.ref_state.to_rc()?, &self.config.to_rc()?, self.mint_url.as_str());
                    if btc_mint.check_btc_coinage_record(tx)? {
                        result = btc_mint.execute_btc_coinage_record(tx).await;
                    } else {
                        result = Err(crate::meta_err!(ERROR_INVALID));
                    }
                },
                MetaTxBody::CreateMinerGroup(ref tx) => {
                    result = self.execute_create_miners_tx(&mut context, &mut fee_counter, tx).await;
                },
                MetaTxBody::WithdrawFromSubChain(ref withdraw_tx) => {
                    result = self.execute_withdraw_from_sub_chain(tx, &mut context, &mut fee_counter, withdraw_tx).await;
                },
                MetaTxBody::SubChainCoinageRecord(ref tx) => {
                    result = self.execute_subchain_coinage(&mut context, &mut fee_counter, tx).await;
                },
                MetaTxBody::CreateSubChainAccount(miner_group) => {
                    result = self.execute_subchain_create_account(&mut context, &mut fee_counter, miner_group).await
                },
                MetaTxBody::UpdateSubChainAccount(miner_group) => {
                    result = self.execute_subchain_update_account(&mut context, &mut fee_counter, miner_group).await
                },
                MetaTxBody::SubChainWithdraw(withdraw_tx) => {
                    result = self.execute_subchain_withdraw(tx, &mut context, &mut fee_counter, withdraw_tx).await
                },
                MetaTxBody::Extension(ref extension_tx) => {
                    let extension = MetaExtensionManager::get_extension(&extension_tx.extension_id);
                    if extension.is_some() {
                        let result2 = extension.unwrap().execute_tx(&mut context, &mut fee_counter, tx, &extension_tx.tx_data).await;
                        match result2 {
                            Ok(mut result_logs) => {
                                logs.append(&mut result_logs);
                                result = Ok(());
                            },
                            Err(e) => {
                                result = Err(e);
                            }
                        }
                    } else {
                        error!("execute unsupport extension transcation");
                        result = Err(crate::meta_err!(ERROR_UNKNOWN_EXTENSION_TX));
                    }
                },
                MetaTxBody::CreateContract(inner_tx) => {
                    let result2 = self.execute_create_contract(&mut context, &mut fee_counter, inner_tx, &mut backend, &self.evm_config).await;
                    match result2 {
                        Ok((reason, address2, value2, mut logs2)) => {
                            logs.append(&mut logs2);
                            address = address2;
                            return_value = value2;
                            // 在这里修改result_code作为返回值
                            result_code = evm_reason_to_code(reason);
                        },
                        Err(e) => {
                            result = Err(e);
                        }
                    }
                },
                MetaTxBody::CreateContract2(inner_tx) => {
                    let result2 = self.execute_create2_contract(&mut context, &mut fee_counter, inner_tx, &mut backend, &self.evm_config).await;
                    match result2 {
                        Ok((reason, address2, value2, mut logs2)) => {
                            logs.append(&mut logs2);
                            address = address2;
                            return_value = value2;
                            // 在这里修改result_code作为返回值
                            result_code = evm_reason_to_code(reason);
                        },
                        Err(e) => {
                            result = Err(e);
                        }
                    }
                },
                MetaTxBody::CallContract(inner_tx) => {
                    let result2 = self.execute_call_contract(&mut context, &mut fee_counter, inner_tx, &mut backend, &self.evm_config).await;
                    match result2 {
                        Ok((reason, value2, mut logs2)) => {
                            logs.append(&mut logs2);
                            return_value = value2;
                            // 在这里修改result_code作为返回值
                            result_code = evm_reason_to_code(reason);
                        },
                        Err(e) => {
                            result = Err(e);
                        }
                    }
                },
                MetaTxBody::SetBenefi(tx) => {
                    result = self.execute_set_benefi_tx(&mut context, &mut fee_counter, tx).await;
                },
                MetaTxBody::NFTCreate(tx) => {
                    result = self.execute_nft_create(&mut context, tx).await;
                },
                MetaTxBody::NFTCreate2(tx) => {
                    result = self.execute_nft_create2(&mut context, tx).await;
                },
                MetaTxBody::NFTAuction(tx) => {
                    result = self.execute_nft_auction(&mut context, tx).await;
                },
                MetaTxBody::NFTBid(tx) => {
                    result = self.execute_nft_bid(&mut context, tx).await;
                },
                MetaTxBody::NFTBuy(tx) => {
                    result = self.execute_nft_buy(&mut context, tx).await;
                },
                MetaTxBody::NFTSell(tx) => {
                    result = self.execute_nft_sell(&mut context, tx).await;
                },
                MetaTxBody::NFTSell2(tx) => {
                    result = self.execute_nft_sell2(&mut context, tx).await;
                },
                MetaTxBody::NFTApplyBuy(tx) => {
                    result = self.execute_nft_apply_buy(&mut context, tx).await;
                },
                MetaTxBody::NFTCancelApplyBuyTx(tx) => {
                    result = self.execute_nft_cancel_apply_buy(&mut context, tx).await;
                },
                MetaTxBody::NFTAgreeApply(tx) => {
                    result = self.execute_nft_agree_apply_buy(&mut context, tx).await;
                }
                MetaTxBody::NFTLike(tx) => {
                    result = self.execute_nft_like(&mut context, tx).await;
                },
                MetaTxBody::NFTCancelSellTx(tx) => {
                    result = self.execute_nft_cancel_sell(&mut context, tx).await;
                },
                MetaTxBody::NFTSetNameTx(tx) => {
                    result = self.execute_nft_set_name(&mut context, tx).await;
                },
                MetaTxBody::NFTTrans(tx) => {
                    result = self.execute_nft_trans(&mut context, tx).await;
                }
                _ => {
                    error!("execute unsupport transcation");
                    result = Err(meta_err!(ERROR_INVALID));
                }
            }
            if result.is_err() {
                break;
            }
        }

        if let Err(err) = result {
            log::info!("tx {} execute err {}", tx.desc().calculate_id().to_string(), &err);
            self.ref_state.to_rc()?.rollback().await?;
            if let BuckyErrorCode::MetaError(code) = err.code() {
                if code == ERROR_EXCEPTION {
                    error!("execute tx failed for exception, err:{}", &err);
                    return Err(err);
                }
                result_code = code;
            } else {
                result_code = err.code().into();
            }
        } else {
            self.ref_state.to_rc()?.commit().await?;
        }

        // TODO: 暂时扣除所有手续费
        let _ = fee_counter.cost(tx.desc().content().max_fee);

        let fee_used = fee_counter.fee_used() as i64 * tx.desc().content().gas_price as i64;
        let fee_change = (tx.desc().content().max_fee - fee_counter.fee_used()) as i64 * tx.desc().content().gas_price as i64;

        if fee_change != 0 {
            self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(tx.desc().content().gas_coin_id), context.caller().id(), fee_change).await?;
        }
        self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(tx.desc().content().gas_coin_id), owner_block.coinbase(), fee_used).await?;

        info!("execute tx result {}", result_code);
        let mut receipt = Receipt::new(result_code as u32, fee_counter.fee_used());
        receipt.address = address;
        receipt.return_value = return_value;
        receipt.logs = logs;
        Ok(receipt)
    }
}

#[cfg(test)]
mod tx_executor_tests {
    use crate::{BlockDescContent, NFTAuction, sql_storage_tests, State};
    use crate::executor::context::{Config, UnionWithdrawManager};
    use crate::events::event_manager::EventManager;
    use crate::rent::rent_manager::RentManager;
    use crate::name_auction::auction::Auction;
    use cyfs_base::*;
    use cyfs_base_meta::*;
    use crate::executor::tx_executor::TxExecutor;
    use std::str::FromStr;
    use std::convert::TryFrom;
    use std::time::Duration;
    use cyfs_core::{NFTList, NFTListObject};

    #[test]
    fn test_name_state() {
        async_std::task::block_on(async {
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
                                           &union_withdraw_manager, &nft_auction, "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true, None);

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
            let mut nonce2 = 1;
            let ctid = CoinTokenId::Coin(0);
            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            let name_start_block = 2;
            for i in 1..1000 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 1 {
                    state.inc_balance(&ctid, &id1, 300).await.unwrap();
                    state.inc_balance(&ctid, &id2, 300).await.unwrap();
                } else if i == name_start_block {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::BidName(BidNameTx {
                            name: "test".to_string(),
                            owner: None,
                            name_price: 1,
                            price: 1
                        })
                        , Vec::new()).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);
                } else if i == name_start_block + config.max_auction_stop_interval() + 1 {
                    let ret = state.get_name_info("test").await;
                    assert!(ret.is_ok());
                    let name_info = ret.as_ref().unwrap().as_ref().unwrap().0.clone();
                    assert_eq!(name_info.owner.unwrap(), id1);
                    if let NameLink::ObjectLink(id) = name_info.record.link {
                        assert_eq!(id, id1);
                    } else {
                        assert!(false);
                    }

                    let tx = MetaTx::new(
                        nonce1
                        ,TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::AuctionName(AuctionNameTx {
                            name: "test".to_string(),
                            price: 100
                        })
                        , Vec::new()).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);

                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::Device(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::BidName(BidNameTx {
                            name: "test".to_string(),
                            owner: None,
                            name_price: 150,
                            price: 100
                        })
                        , Vec::new()).build();
                    nonce2 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);
                } else if i == name_start_block + (config.max_auction_stop_interval() + 1) * 2 {
                    let ret = state.get_name_info("test").await;
                    assert!(ret.is_ok());
                    let name_info = ret.as_ref().unwrap().as_ref().unwrap().0.clone();
                    assert_eq!(name_info.owner.unwrap(), id2);
                    if let NameLink::ObjectLink(id) = name_info.record.link {
                        assert_eq!(id, id2);
                    } else {
                        assert!(false);
                    }
                } else if i == name_start_block + (config.max_auction_stop_interval() + 1) * 2 + config.get_rent_cycle() * 2 + 1 {
                    let balance = state.get_balance(&id2, &ctid).await.unwrap();
                    assert_eq!(balance, 0);

                    let ret = state.get_name_extra("test").await;
                    assert!(ret.is_ok());
                    let name_extra = ret.unwrap();
                    assert_eq!(name_extra.rent_arrears, 50);

                    let ret = state.get_name_info("test").await;
                    assert!(ret.is_ok());
                    let name_state = ret.as_ref().unwrap().as_ref().unwrap().1;
                    assert_eq!(name_state, NameState::Lock);

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::TransBalance(TransBalanceTx {
                            ctid: CoinTokenId::Coin(0),
                            to: vec![(id2, 200)],
                        })
                        , Vec::new(),
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();

                    let balance = state.get_balance(&id2, &ctid).await.unwrap();
                    assert_eq!(balance, 150);

                    let ret = state.get_name_extra("test").await;
                    assert!(ret.is_ok());
                    let name_extra = ret.unwrap();
                    assert_eq!(name_extra.rent_arrears, 0);

                    let ret = state.get_name_info("test").await;
                    assert!(ret.is_ok());
                    let name_state = ret.as_ref().unwrap().as_ref().unwrap().1;
                    assert_eq!(name_state, NameState::Normal);
                }

                event_manager.run_event(&new).await.unwrap();
                prev = new;
            }

        });
    }

    #[test]
    fn test_delete_desc() {
        async_std::task::block_on(async {
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
                                           &union_withdraw_manager, &nft_auction, "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true, None);

            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn6").unwrap();

            let private_key1 = PrivateKey::generate_rsa(1024).unwrap();
            let device1: Device = Device::new(
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
            let device2: Device = Device::new(
                None
                , UniqueId::default()
                , Vec::new()
                , Vec::new()
                , Vec::new()
                , private_key2.public()
                , Area::default()
                , DeviceCategory::OOD).build();
            let id2 = device2.desc().calculate_id();

            let chunk_list = vec![ChunkId::default()];
            let chunk_list = ChunkList::ChunkInList(chunk_list);
            let file: File = File::new(
                id1.clone()
                , 1024
                , HashValue::default()
                , chunk_list).build();
            let file_id = file.desc().calculate_id();

            let mut nonce1 = 1;
            let mut nonce2 = 1;
            let ctid = CoinTokenId::Coin(0);
            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1 as i32..1000 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 1 {
                    state.inc_balance(&ctid, &id1, 300).await.unwrap();
                    state.inc_balance(&ctid, &id2, 300).await.unwrap();

                    let saved_obj = SavedMetaObject::try_from(StandardObject::Device(device1.clone())).unwrap();
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 10,
                        })
                        , saved_obj.to_vec().unwrap(),
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();

                    let desc_ret = state.get_obj_desc(&id1).await;
                    assert!(desc_ret.is_ok());

                    let saved_obj = SavedMetaObject::try_from(StandardObject::File(file.clone())).unwrap();
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 10
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();

                    let desc_ret = state.get_obj_desc(&file_id).await;
                    assert!(desc_ret.is_ok());
                } else if i == 2 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::RemoveDesc(RemoveDescTx {
                            id: id1
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    let desc_ret = state.get_obj_desc(&id1).await;
                    assert!(!desc_ret.is_ok());

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::RemoveDesc(RemoveDescTx {
                            id: file_id.clone()
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    let desc_ret = state.get_obj_desc(&file_id).await;
                    assert!(!desc_ret.is_ok());
                } else if i == 3 {
                    let saved_obj = SavedMetaObject::try_from(StandardObject::File(file.clone())).unwrap();
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 10
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();

                    let desc_ret = state.get_obj_desc(&file_id).await;
                    assert!(desc_ret.is_ok());

                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::Device(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::TransBalance(TransBalanceTx {
                            ctid: CoinTokenId::Coin(0),
                            to: vec![(file_id.clone(), 20)]
                        })
                        , Vec::new()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();

                    let balance = state.get_balance(&file_id, &ctid).await.unwrap();
                    assert_eq!(balance, 20);
                } else if i == 4 {
                    let tx = MetaTx::new(
                        nonce2,
                        TxCaller::try_from(&StandardObject::Device(device2.clone())).unwrap(),
                        0,
                        0,
                        0,
                        None,
                        MetaTxBody::WithdrawToOwner(WithdrawToOwner {
                            ctid: CoinTokenId::Coin(0),
                            id: file_id.clone(),
                            value: 20
                        }),
                        Vec::new()
                    ).build();
                    nonce2 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_INVALID);

                    let device1_balance = state.get_balance(&id1, &ctid).await.unwrap();

                    let tx = MetaTx::new(
                        nonce1,
                        TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap(),
                        0,
                        0,
                        0,
                        None,
                        MetaTxBody::WithdrawToOwner(WithdrawToOwner {
                            ctid: CoinTokenId::Coin(0),
                            id: file_id.clone(),
                            value: 20
                        }),
                        Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();

                    let balance = state.get_balance(&file_id, &ctid).await.unwrap();
                    assert_eq!(balance, 0);
                    let device1_new_balance = state.get_balance(&id1, &ctid).await.unwrap();
                    assert_eq!(device1_new_balance - device1_balance, 20);

                // } else if i == 4 { //TODO:删除其他人充值余额存在时不能删除case
                //     let tx = MetaTx::new(
                //         nonce1
                //         , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                //         , 0
                //         , 0
                //         , 0
                //         , None
                //         , TxBody::RemoveDesc(RemoveDescTx {
                //             id: file_id.clone()
                //         })
                //         , Vec::new()
                //     ).build();
                //     nonce1 += 1;
                //     let ret = executor.execute(&new, &tx, None).await;
                //     assert!(ret.is_ok());
                //     assert_eq!(ret.as_ref().unwrap().result, ERROR_OTHER_CHARGED);
                //
                //     let desc_ret = state.get_obj_desc(&file_id).await;
                //     assert!(desc_ret.is_ok());
                } else if i == 204 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::RemoveDesc(RemoveDescTx {
                            id: file_id.clone()
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);

                    let desc_ret = state.get_obj_desc(&file_id).await;
                    assert!(!desc_ret.is_ok());
                }
                event_manager.run_event(&new).await.unwrap();
                prev = new;
            }

        });
    }

    #[test]
    fn test_change_rent_cycle() {
        async_std::task::block_on(async {
            let state = sql_storage_tests::create_state().await;
            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();

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

            let ctid = CoinTokenId::Coin(0);
            let mut nonce1 = 1;
            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1 as i32..1000 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                let config = Config::new(&state).unwrap();
                let ret = state.create_cycle_event_table(config.get_rent_cycle()).await;
                assert!(ret.is_ok());

                let event_manager = EventManager::new(&state, &config);
                let rent_manager = RentManager::new(&state, &config, &event_manager);
                let auction = Auction::new(&state, &config, &rent_manager, &event_manager);
                let union_withdraw_manager = UnionWithdrawManager::new(&state, &config, &event_manager);
                let nft_auction = NFTAuction::new(&state, &config, &event_manager);
                let executor = TxExecutor::new(&state, &config, &rent_manager, &auction, &event_manager,
                                               &union_withdraw_manager, &nft_auction,  "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true, None);

                if i == 1 {
                    state.inc_balance(&ctid, &id1, 300).await.unwrap();
                    let saved_obj = SavedMetaObject::try_from(StandardObject::Device(device1.clone())).unwrap();
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 10
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                } else if i == 2 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::SetConfig(SetConfigTx {
                            key: "rent_cycle".to_string(),
                            value: "60".to_string()
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                } else if i == 61 {
                    let balance = state.get_balance(&id1, &ctid).await.unwrap();
                    assert_eq!(balance, 290);
                } else if i == 121 {
                    let balance = state.get_balance(&id1, &ctid).await.unwrap();
                    assert_eq!(balance, 280);
                }
                event_manager.run_event(&new).await;
                prev = new;
            }

        });
    }

    #[test]
    fn test_union() {
        async_std::task::block_on(async {
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
                                           &union_withdraw_manager, &nft_auction, "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true, None);

            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();

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

            let union = UnionAccount::new(id1.clone(), id2.clone(), 0).build();

            let mut nonce1 = 1;
            let mut nonce2 = 1;
            let ctid = CoinTokenId::Coin(0);
            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..1000 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 1 {
                    let mut create_union_tx = CreateUnionTx {
                        body: CreateUnionBody {
                            account: union.clone(),
                            ctid,
                            left_balance: 100,
                            right_balance: 150
                        },
                        signs: vec![]
                    };

                    //验证没有相关用户desc数据时创建union account时case
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateUnion(create_union_tx.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_CANT_FIND_LEFT_USER_DESC);

                    let saved_obj = SavedMetaObject::try_from(StandardObject::Device(device1.clone())).unwrap();
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx{
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 10
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());

                    let saved_obj = SavedMetaObject::try_from(StandardObject::Device(device2.clone())).unwrap();
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::Device(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx{
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 10
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce2 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());

                    //验证没有签名创建union account case
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateUnion(create_union_tx.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_INVALID);

                    let ret = create_union_tx.sign(ObjectLink {obj_id: id1, obj_owner: None}, private_key1.clone());
                    assert!(ret.is_ok());

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateUnion(create_union_tx.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_INVALID);

                    let ret = create_union_tx.sign(ObjectLink { obj_id: id2, obj_owner: None}, private_key2.clone());
                    assert!(ret.is_ok());

                    //验证没有余额创建union account case
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateUnion(create_union_tx.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_NO_ENOUGH_BALANCE);

                    let ret = state.inc_balance(&ctid, &union.desc().content().left(), 300).await;
                    assert!(ret.is_ok());

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateUnion(create_union_tx.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_NO_ENOUGH_BALANCE);

                    let ret = state.inc_balance(&ctid, &union.desc().content().right(), 300).await;
                    assert!(ret.is_ok());

                    //验证有足够余额创建union account case
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateUnion(create_union_tx.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);

                    let balance = state.get_balance(&union.desc().content().right(), &ctid).await;
                    assert!(balance.is_ok());
                    assert_eq!(balance.unwrap(), 150);

                    let balance = state.get_balance(&union.desc().content().left(), &ctid).await;
                    assert!(balance.is_ok());
                    assert_eq!(balance.unwrap(), 200);

                    //测试提交闪电网络交易case
                    let mut deviate = DeviateUnionTx {
                        body: DeviateUnionBody {
                            ctid,
                            seq: 2,
                            deviation: -10,
                            union: union.desc().calculate_id()
                        },
                        signs: vec![]
                    };
                    let ret = deviate.sign(ObjectLink {obj_id: id1.clone(), obj_owner: None}, private_key1.clone());
                    assert!(ret.is_ok());

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::DeviateUnion(deviate.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_INVALID);

                    let ret = deviate.sign(ObjectLink {obj_id: id2.clone(), obj_owner: None}, private_key2.clone());
                    assert!(ret.is_ok());

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::DeviateUnion(deviate.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);

                    let ret = state.get_union_balance(&ctid, &union.desc().calculate_id()).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().deviation, -10);
                    assert_eq!(ret.as_ref().unwrap().left, 100);
                    assert_eq!(ret.as_ref().unwrap().right, 150);
                    assert_eq!(ret.as_ref().unwrap().total, 250);


                    let mut deviate = DeviateUnionTx {
                        body: DeviateUnionBody {
                            ctid,
                            seq: 2,
                            deviation: -10,
                            union: union.desc().calculate_id()
                        },
                        signs: vec![]
                    };
                    let ret = deviate.sign(ObjectLink {obj_id: id1.clone(), obj_owner: None}, private_key1.clone());
                    assert!(ret.is_ok());
                    let ret = deviate.sign(ObjectLink {obj_id: id2.clone(), obj_owner: None}, private_key2.clone());
                    assert!(ret.is_ok());
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::DeviateUnion(deviate.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_ACCESS_DENIED);

                    let mut deviate = DeviateUnionTx {
                        body: DeviateUnionBody {
                            ctid,
                            seq: 3,
                            deviation: -20,
                            union: union.desc().calculate_id()
                        },
                        signs: vec![]
                    };
                    let ret = deviate.sign(ObjectLink {obj_id: id1.clone(), obj_owner: None}, private_key1.clone());
                    assert!(ret.is_ok());
                    let ret = deviate.sign(ObjectLink {obj_id: id2.clone(), obj_owner: None}, private_key2.clone());
                    assert!(ret.is_ok());
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::DeviateUnion(deviate.clone())
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);

                    let ret = state.get_union_balance(&ctid, &union.desc().calculate_id()).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().deviation, -20);
                    assert_eq!(ret.as_ref().unwrap().left, 100);
                    assert_eq!(ret.as_ref().unwrap().right, 150);
                    assert_eq!(ret.as_ref().unwrap().total, 250);

                    let mut device = device1.clone();
                    let mut nonce = &mut nonce1;
                    if id2 == union.desc().content().left().clone() {
                        device = device2.clone();
                        nonce = &mut nonce2;
                    }
                    let tx = MetaTx::new(
                        *nonce
                        , TxCaller::try_from(&StandardObject::Device(device.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::WithdrawFromUnion(WithdrawFromUnionTx {
                            ctid,
                            union: union.desc().calculate_id(),
                            value: 50
                        })
                        , Vec::new()
                    ).build();
                    *nonce = *nonce + 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);

                } else if i == 1 + config.union_withdraw_interval() {
                    let ret = state.get_union_balance(&ctid, &union.desc().calculate_id()).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().deviation, -20);
                    assert_eq!(ret.as_ref().unwrap().left, 100);
                    assert_eq!(ret.as_ref().unwrap().right, 150);
                    assert_eq!(ret.as_ref().unwrap().total, 250);
                } else if i == 1 + config.union_withdraw_interval() + 1 {
                    let ret = state.get_union_balance(&ctid, &union.desc().calculate_id()).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().deviation, -20);
                    assert_eq!(ret.as_ref().unwrap().left, 50);
                    assert_eq!(ret.as_ref().unwrap().right, 150);
                    assert_eq!(ret.as_ref().unwrap().total, 200);

                    let mut deviate = DeviateUnionTx {
                        body: DeviateUnionBody {
                            ctid,
                            seq: 4,
                            deviation: -50,
                            union: union.desc().calculate_id()
                        },
                        signs: vec![]
                    };
                    let ret = deviate.sign(ObjectLink {obj_id: id1.clone(), obj_owner: None}, private_key1.clone());
                    assert!(ret.is_ok());
                    let ret = deviate.sign(ObjectLink {obj_id: id2.clone(), obj_owner: None}, private_key2.clone());
                    assert!(ret.is_ok());
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::Device(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::DeviateUnion(deviate.clone())
                        , Vec::new()
                    ).build();
                    nonce2 += 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);

                    let ret = state.get_union_balance(&ctid, &union.desc().calculate_id()).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().deviation, -50);
                    assert_eq!(ret.as_ref().unwrap().left, 50);
                    assert_eq!(ret.as_ref().unwrap().right, 150);
                    assert_eq!(ret.as_ref().unwrap().total, 200);

                    let mut device = device1.clone();
                    let mut nonce = &mut nonce1;
                    if id2 == union.desc().content().left().clone() {
                        device = device2.clone();
                        nonce = &mut nonce2;
                    }
                    let tx = MetaTx::new(
                        *nonce
                        , TxCaller::try_from(&StandardObject::Device(device.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::WithdrawFromUnion(WithdrawFromUnionTx {
                            ctid,
                            union: union.desc().calculate_id(),
                            value: 40
                        })
                        , Vec::new()
                    ).build();
                    *nonce = *nonce + 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);
                } else if  i == 1 + (config.union_withdraw_interval() + 1)*2 {
                    let ret = state.get_union_balance(&ctid, &union.desc().calculate_id()).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().deviation, -50);
                    assert_eq!(ret.as_ref().unwrap().left, 50);
                    assert_eq!(ret.as_ref().unwrap().right, 150);
                    assert_eq!(ret.as_ref().unwrap().total, 200);

                    let mut device = device1.clone();
                    let mut nonce = &mut nonce1;
                    if id2 == union.desc().content().right().clone() {
                        device = device2.clone();
                        nonce = &mut nonce2;
                    }
                    let tx = MetaTx::new(
                        *nonce
                        , TxCaller::try_from(&StandardObject::Device(device.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::WithdrawFromUnion(WithdrawFromUnionTx {
                            ctid,
                            union: union.desc().calculate_id(),
                            value: 200
                        })
                        , Vec::new()
                    ).build();
                    *nonce = *nonce + 1;
                    let ret = executor.execute(&new, &tx, None).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().result as u16, ERROR_SUCCESS);
                } else if  i == 1 + (config.union_withdraw_interval() + 1)*3 {
                    let ret = state.get_union_balance(&ctid, &union.desc().calculate_id()).await;
                    assert!(ret.is_ok());
                    assert_eq!(ret.as_ref().unwrap().deviation, -50);
                    assert_eq!(ret.as_ref().unwrap().left, 50);
                    assert_eq!(ret.as_ref().unwrap().right, -50);
                    assert_eq!(ret.as_ref().unwrap().total, 0);
                }

                let ret = event_manager.run_event(&new).await;
                if i == 1 + config.union_withdraw_interval() * 2 + 1 {
                    assert!(ret.is_err());
                }

                prev = new;
            }

        });
    }

    #[test]
    fn test_save_data() {
        async_std::task::block_on(async {
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
                                           &union_withdraw_manager, &nft_auction, "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true, None);

            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();

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
            let _id2 = device2.desc().calculate_id();

            let chunk_list = vec![ChunkId::default()];
            let chunk_list = ChunkList::ChunkInList(chunk_list);
            let file = File::new(
                id1.clone()
                , 1024
                , HashValue::default()
                , chunk_list).build();
            let _file_id = file.desc().calculate_id();

            let data_id = ObjectId::from_str("9tGpLNnGXCbjRMdS6ara6SzSiNMwNAJZ4YUGzU7h6uXc").unwrap();
            let data_id2 = ObjectId::from_str("9tGpLNnGXCbjRMdS6ara6SzSiNMwNAJZ4YUGzU7h6uXd").unwrap();
            let mut nonce1 = 1;
            let mut _nonce2: i32 = 1;
            let _ctid = CoinTokenId::Coin(0);
            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..1000 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 1 {
                    let data_tx = Data {
                        id: data_id.clone(),
                        data: vec![]
                    };
                    let saved_obj = SavedMetaObject::Data(data_tx);
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 0
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();

                    let data_tx = Data {
                        id: data_id2.clone(),
                        data: vec![1, 2]
                    };
                    let saved_obj = SavedMetaObject::Data(data_tx);
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::Device(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 0
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                } else if i == 3 {
                    let ret = state.get_obj_desc(&data_id).await;
                    assert!(ret.is_ok());
                    let ret = state.get_obj_desc(&data_id2).await;
                    assert!(ret.is_ok());
                    if let SavedMetaObject::Data(data) = ret.unwrap() {
                        assert_eq!(data.data.len(), 2);
                        assert_eq!(data.data[0], 1);
                        assert_eq!(data.data[1], 2);
                    } else {
                        assert!(false)
                    }
                }
                event_manager.run_event(&new).await.unwrap();
                prev = new;
            }


        });
    }

    lazy_static::lazy_static! {
        static ref INIT_LOG: std::sync::Mutex<bool> = std::sync::Mutex::new(false);
    }
    pub fn init_test_log() {
        let mut init_log = INIT_LOG.lock().unwrap();
        if !*init_log {
            cyfs_util::init_log("test_dsg_client", None);
        }
        *init_log = true;
    }

    fn rand_hash() -> HashValue {
        let mut node = [0u8; 32];
        for i in 0..4 {
            let r = rand::random::<u64>();
            node[i * 8..(i + 1) * 8].copy_from_slice(&r.to_be_bytes());
        }
        HashValue::from(&node)
    }

    #[test]
    fn test_nft() {
        init_test_log();
        async_std::task::block_on(async {
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
                                           &union_withdraw_manager, &nft_auction, "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true, None);

            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();

            let private_key1 = PrivateKey::generate_rsa(1024).unwrap();
            let device1 = People::new(
                None
                , Vec::new()
                , private_key1.public()
                , None
                , None
                , None).build();
            let id1 = device1.desc().calculate_id();

            let private_key2 = PrivateKey::generate_rsa(1024).unwrap();
            let device2 = People::new(
                None
                , Vec::new()
                , private_key2.public()
                , None
                , None
                , None).build();
            let id2 = device2.desc().calculate_id();

            let chunk_list = vec![ChunkId::default()];
            let chunk_list = ChunkList::ChunkInList(chunk_list);
            let file = File::new(
                id1.clone()
                , 1024
                , rand_hash()
                , chunk_list.clone()).create_time(bucky_time_now()).build();
            let file_id = file.desc().calculate_id();
            log::info!("create nft {}", file_id.to_string());

            let mut nft_list = Vec::new();
            let mut names = Vec::new();
            for i in 0..4 {
                async_std::task::sleep(Duration::from_millis(100)).await;
                let now = bucky_time_now();
                let file = File::new(
                    id1.clone()
                    , 1024
                    , rand_hash()
                    , chunk_list.clone()).create_time(now).build();
                nft_list.push(file.into_desc());
                names.push(format!("test_{}", i));
            }
            let nft_list = NFTList::new(id1.clone(), nft_list);
            let nft_list_id = nft_list.desc().calculate_id();

            let mut nft_list2 = Vec::new();
            let mut names2 = Vec::new();
            nft_list2.push(file.desc().clone());
            names2.push("test_0".to_string());
            for i in 1..4 {
                async_std::task::sleep(Duration::from_millis(100)).await;
                let file = File::new(
                    id1.clone()
                    , 1024
                    , rand_hash()
                    , chunk_list.clone()).create_time(bucky_time_now()).build();
                nft_list2.push(file.into_desc());
                names2.push(format!("test_{}", i));
            }
            let nft_list2 = NFTList::new(id1.clone(), nft_list2);

            let mut nft_list3 = Vec::new();
            let mut names3 = Vec::new();
            for i in 0..4 {
                async_std::task::sleep(Duration::from_millis(100)).await;
                let now = bucky_time_now();
                let file = File::new(
                    id1.clone()
                    , 1024
                    , rand_hash()
                    , chunk_list.clone()).create_time(now).build();
                nft_list3.push(file.into_desc());
                names3.push(format!("test_{}", i));
            }
            let nft_list3 = NFTList::new(id1.clone(), nft_list3);
            let nft_list_id3 = nft_list.desc().calculate_id();

            //
            // let mut nft_list4 = Vec::new();
            // let mut names4 = Vec::new();
            // for i in 0..4 {
            //     async_std::task::sleep(Duration::from_millis(100)).await;
            //     let now = bucky_time_now();
            //     let file = File::new(
            //         id1.clone()
            //         , 1024
            //         , rand_hash()
            //         , chunk_list.clone()).create_time(now).build();
            //     nft_list4.push(file.into_desc());
            //     names4.push(format!("test_{}", i));
            // }
            // let nft_list4 = NFTList::new(id1.clone(), nft_list4);
            // let nft_list_id4 = nft_list.desc().calculate_id();

            let mut nft_list5 = Vec::new();
            let mut names5 = Vec::new();
            let mut sell_list5 = Vec::new();
            for i in 0..4 {
                async_std::task::sleep(Duration::from_millis(100)).await;
                let now = bucky_time_now();
                let file = File::new(
                    id1.clone()
                    , 1024
                    , rand_hash()
                    , chunk_list.clone()).create_time(now).build();
                nft_list5.push(file.into_desc());
                names5.push(format!("test_{}", i));
                sell_list5.push(NFTState::Selling((1, CoinTokenId::Coin(0), 0)));
            }
            let nft_list5 = NFTList::new(id1.clone(), nft_list5);
            let nft_list_id5 = nft_list.desc().calculate_id();
            //
            // let mut nft_list6 = Vec::new();
            // let mut names6 = Vec::new();
            // for i in 0..4 {
            //     async_std::task::sleep(Duration::from_millis(100)).await;
            //     let now = bucky_time_now();
            //     let file = File::new(
            //         id1.clone()
            //         , 1024
            //         , rand_hash()
            //         , chunk_list.clone()).create_time(now).build();
            //     nft_list6.push(file.into_desc());
            //     names6.push(format!("test_{}", i));
            // }
            // let nft_list6 = NFTList::new(id1.clone(), nft_list6);
            // let nft_list_id6 = nft_list.desc().calculate_id();

            let data_id = ObjectId::from_str("9tGpLNnGXCbjRMdS6ara6SzSiNMwNAJZ4YUGzU7h6uXc").unwrap();
            let data_id2 = ObjectId::from_str("9tGpLNnGXCbjRMdS6ara6SzSiNMwNAJZ4YUGzU7h6uXd").unwrap();
            let mut nonce1 = 1;
            let mut nonce2 = 1;
            let mut balance1 = 0;
            let mut balance2 = 0;
            let _ctid = CoinTokenId::Coin(0);
            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..1000 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 1 {
                    let saved_obj = SavedMetaObject::People(device1.clone());
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 0
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    balance1 = state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap();

                    let saved_obj = SavedMetaObject::People(device2.clone());
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 0
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    balance2 = state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap();
                } else if i == 2 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate(NFTCreateTx {
                            desc: NFTDesc::FileDesc(file.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Normal
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    state.nft_get(&file_id).await.unwrap();

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate2(NFTCreateTx2 {
                            desc: NFTDesc::ListDesc(nft_list.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Normal,
                            sub_names: names.clone(),
                            sub_states: vec![]
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    state.nft_get(&nft_list.desc().calculate_id()).await.unwrap();

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate2(NFTCreateTx2 {
                            desc: NFTDesc::ListDesc(nft_list2.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Normal,
                            sub_names: names2.clone(),
                            sub_states: vec![]
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    let ret = state.nft_get(&nft_list2.desc().calculate_id()).await;
                    assert!(ret.is_err());

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate2(NFTCreateTx2 {
                            desc: NFTDesc::ListDesc(nft_list3.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Auctioning((1, CoinTokenId::Coin(0), 100)),
                            sub_names: names.clone(),
                            sub_states: vec![]
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    state.nft_get(&nft_list3.desc().calculate_id()).await.unwrap();

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate2(NFTCreateTx2 {
                            desc: NFTDesc::ListDesc(nft_list5.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Selling((0, CoinTokenId::Coin(0), 0)),
                            sub_names: names5.clone(),
                            sub_states: vec![]
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    let ret = state.nft_get(&nft_list5.desc().calculate_id()).await;
                    assert!(ret.is_err());

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate2(NFTCreateTx2 {
                            desc: NFTDesc::ListDesc(nft_list5.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Selling((1, CoinTokenId::Coin(0), 0)),
                            sub_names: names.clone(),
                            sub_states: sell_list5.clone()
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    state.nft_get(&nft_list5.desc().calculate_id()).await.unwrap();

                } else if i == 3 {
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTApplyBuy(NFTApplyBuyTx {
                            nft_id: file_id.clone(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0)
                        })
                        , Vec::new()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2 - 1);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1);

                } else if i == 4 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTAgreeApply(NFTAgreeApplyTx {
                            nft_id: file_id.clone(),
                            user_id: id2.clone(),
                        })
                        , Vec::new(),
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();

                    let beneficiary = state.get_beneficiary(&file_id).await.unwrap();
                    assert_eq!(beneficiary, id2);
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2 - 1);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1 + 1);
                } else if i == 5 {
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTAuction(NFTAuctionTx {
                            nft_id: file_id.clone(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0),
                            duration_block_num: 5
                        })
                        , Vec::new()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                } else if i == 6 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTBid(NFTBidTx {
                            nft_id: file_id.clone(),
                            price: 2,
                            coin_id: CoinTokenId::Coin(0),
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1 - 1);
                } else if i == 11 {
                    let beneficiary = state.get_beneficiary(&file_id).await.unwrap();
                    assert_eq!(beneficiary, id1);
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2 + 1);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1 - 1);
                } else if i == 12 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTSell(NFTSellTx {
                            nft_id: file_id.clone(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0),
                            duration_block_num: 0
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                } else if i == 13 {
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTBuy(NFTBuyTx {
                            nft_id: file_id.clone(),
                            price: 2,
                            coin_id: CoinTokenId::Coin(0),
                        })
                        , Vec::new()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    let beneficiary = state.get_beneficiary(&file_id).await.unwrap();
                    assert_eq!(beneficiary, id2);
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1);
                }
                event_manager.run_event(&new).await.unwrap();
                prev = new;
            }


        });
    }

    #[test]
    fn test_nft_list_normal() {
        init_test_log();
        async_std::task::block_on(async {
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
                                           &union_withdraw_manager, &nft_auction, "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true, None);

            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();

            let private_key1 = PrivateKey::generate_rsa(1024).unwrap();
            let device1 = People::new(
                None
                , Vec::new()
                , private_key1.public()
                , None
                , None
                , None).build();
            let id1 = device1.desc().calculate_id();

            let private_key2 = PrivateKey::generate_rsa(1024).unwrap();
            let device2 = People::new(
                None
                , Vec::new()
                , private_key2.public()
                , None
                , None
                , None).build();
            let id2 = device2.desc().calculate_id();

            let chunk_list = vec![ChunkId::default()];
            let chunk_list = ChunkList::ChunkInList(chunk_list);
            let file = File::new(
                id1.clone()
                , 1024
                , rand_hash()
                , chunk_list.clone()).create_time(bucky_time_now()).build();
            let file_id = file.desc().calculate_id();
            log::info!("create nft {}", file_id.to_string());

            let mut nft_list = Vec::new();
            let mut names = Vec::new();
            for i in 0..4 {
                async_std::task::sleep(Duration::from_millis(100)).await;
                let now = bucky_time_now();
                let file = File::new(
                    id1.clone()
                    , 1024
                    , rand_hash()
                    , chunk_list.clone()).create_time(now).build();
                nft_list.push(file.into_desc());
                names.push(format!("test_{}", i));
            }
            let nft_list = NFTList::new(id1.clone(), nft_list);
            let nft_list_id = nft_list.desc().calculate_id();

            let mut nft_list2 = Vec::new();
            let mut names2 = Vec::new();
            nft_list2.push(file.desc().clone());
            names2.push("test_0".to_string());
            for i in 1..4 {
                async_std::task::sleep(Duration::from_millis(100)).await;
                let file = File::new(
                    id1.clone()
                    , 1024
                    , rand_hash()
                    , chunk_list.clone()).create_time(bucky_time_now()).build();
                nft_list2.push(file.into_desc());
                names2.push(format!("test_{}", i));
            }
            let nft_list2 = NFTList::new(id1.clone(), nft_list2);

            let data_id = ObjectId::from_str("9tGpLNnGXCbjRMdS6ara6SzSiNMwNAJZ4YUGzU7h6uXc").unwrap();
            let data_id2 = ObjectId::from_str("9tGpLNnGXCbjRMdS6ara6SzSiNMwNAJZ4YUGzU7h6uXd").unwrap();
            let mut nonce1 = 1;
            let mut nonce2 = 1;
            let mut balance1 = 0;
            let mut balance2 = 0;
            let _ctid = CoinTokenId::Coin(0);
            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..1000 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 1 {
                    let saved_obj = SavedMetaObject::People(device1.clone());
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 0
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    balance1 = state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap();

                    let saved_obj = SavedMetaObject::People(device2.clone());
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 0
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    balance2 = state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap();
                } else if i == 2 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate(NFTCreateTx {
                            desc: NFTDesc::FileDesc(file.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Normal
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    state.nft_get(&file_id).await.unwrap();

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate2(NFTCreateTx2 {
                            desc: NFTDesc::ListDesc(nft_list.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Normal,
                            sub_names: names.clone(),
                            sub_states: vec![]
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    state.nft_get(&nft_list.desc().calculate_id()).await.unwrap();

                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate2(NFTCreateTx2 {
                            desc: NFTDesc::ListDesc(nft_list2.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Normal,
                            sub_names: names2.clone(),
                            sub_states: vec![]
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    let ret = state.nft_get(&nft_list2.desc().calculate_id()).await;
                    assert!(ret.is_err());
                } else if i == 3 {
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTApplyBuy(NFTApplyBuyTx {
                            nft_id: nft_list.nft_list()[0].calculate_id(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0)
                        })
                        , Vec::new()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2 - 1);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1);
                } else if i == 4 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTAgreeApply(NFTAgreeApplyTx {
                            nft_id: nft_list.nft_list()[0].calculate_id(),
                            user_id: id2.clone(),
                        })
                        , Vec::new(),
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();

                    let beneficiary = state.get_beneficiary(&nft_list.nft_list()[0].calculate_id()).await.unwrap();
                    assert_eq!(beneficiary, id2);
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2 - 1);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1 + 1);
                } else if i == 5 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTSell(NFTSellTx {
                            nft_id: nft_list.desc().calculate_id(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0),
                            duration_block_num: 0
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    let (_, _, nft_state) = state.nft_get(&nft_list.desc().calculate_id()).await.unwrap();
                    assert_eq!(nft_state, NFTState::Normal);


                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTAuction(NFTAuctionTx {
                            nft_id: nft_list.desc().calculate_id(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0),
                            duration_block_num: 10000
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    let (_, _, nft_state) = state.nft_get(&nft_list.desc().calculate_id()).await.unwrap();
                    assert_eq!(nft_state, NFTState::Normal);

                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTAuction(NFTAuctionTx {
                            nft_id: nft_list.nft_list()[0].calculate_id(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0),
                            duration_block_num: 5
                        })
                        , Vec::new()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                } else if i == 6 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTBid(NFTBidTx {
                            nft_id: nft_list.nft_list()[0].calculate_id(),
                            price: 2,
                            coin_id: CoinTokenId::Coin(0),
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1 - 1);
                } else if i == 11 {
                    let beneficiary = state.get_beneficiary(&file_id).await.unwrap();
                    assert_eq!(beneficiary, id1);
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2 + 1);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1 - 1);
                } else if i == 12 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTSell(NFTSellTx {
                            nft_id: nft_list.desc().calculate_id(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0),
                            duration_block_num: 0
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                } else if i == 13 {
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTBuy(NFTBuyTx {
                            nft_id: nft_list.desc().calculate_id(),
                            price: 2,
                            coin_id: CoinTokenId::Coin(0),
                        })
                        , Vec::new()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    let beneficiary = state.get_beneficiary(&nft_list.desc().calculate_id()).await.unwrap();
                    assert_eq!(beneficiary, id2);
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1);
                }
                event_manager.run_event(&new).await.unwrap();
                prev = new;
            }
        });
    }

    #[test]
    fn test_nft_list_auction() {
        init_test_log();
        async_std::task::block_on(async {
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
                                           &union_withdraw_manager, &nft_auction, "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true, None);

            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();

            let private_key1 = PrivateKey::generate_rsa(1024).unwrap();
            let device1 = People::new(
                None
                , Vec::new()
                , private_key1.public()
                , None
                , None
                , None).build();
            let id1 = device1.desc().calculate_id();

            let private_key2 = PrivateKey::generate_rsa(1024).unwrap();
            let device2 = People::new(
                None
                , Vec::new()
                , private_key2.public()
                , None
                , None
                , None).build();
            let id2 = device2.desc().calculate_id();

            let chunk_list = vec![ChunkId::default()];
            let chunk_list = ChunkList::ChunkInList(chunk_list);

            let mut nft_list = Vec::new();
            let mut names = Vec::new();
            for i in 0..4 {
                async_std::task::sleep(Duration::from_millis(100)).await;
                let now = bucky_time_now();
                let file = File::new(
                    id1.clone()
                    , 1024
                    , rand_hash()
                    , chunk_list.clone()).create_time(now).build();
                nft_list.push(file.into_desc());
                names.push(format!("test_{}", i));
            }
            let nft_list = NFTList::new(id1.clone(), nft_list);
            let nft_list_id = nft_list.desc().calculate_id();

            let data_id = ObjectId::from_str("9tGpLNnGXCbjRMdS6ara6SzSiNMwNAJZ4YUGzU7h6uXc").unwrap();
            let data_id2 = ObjectId::from_str("9tGpLNnGXCbjRMdS6ara6SzSiNMwNAJZ4YUGzU7h6uXd").unwrap();
            let mut nonce1 = 1;
            let mut nonce2 = 1;
            let mut balance1 = 0;
            let mut balance2 = 0;
            let _ctid = CoinTokenId::Coin(0);
            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..1000 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 1 {
                    let saved_obj = SavedMetaObject::People(device1.clone());
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 0
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    balance1 = state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap();

                    let saved_obj = SavedMetaObject::People(device2.clone());
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::CreateDesc(CreateDescTx {
                            coin_id: 0,
                            from: None,
                            value: 0,
                            desc_hash: saved_obj.hash().unwrap(),
                            price: 0
                        })
                        , saved_obj.to_vec().unwrap()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    balance2 = state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap();
                } else if i == 2 {
                    let tx = MetaTx::new(
                        nonce1
                        , TxCaller::try_from(&StandardObject::People(device1.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTCreate2(NFTCreateTx2 {
                            desc: NFTDesc::ListDesc(nft_list.desc().clone()),
                            name: "test".to_string(),
                            state: NFTState::Auctioning((1, CoinTokenId::Coin(0), 100)),
                            sub_names: names.clone(),
                            sub_states: vec![]
                        })
                        , Vec::new()
                    ).build();
                    nonce1 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    state.nft_get(&nft_list.desc().calculate_id()).await.unwrap();

                } else if i == 3 {
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTApplyBuy(NFTApplyBuyTx {
                            nft_id: nft_list.nft_list()[0].calculate_id(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0)
                        })
                        , Vec::new()
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1);
                } else if i == 4 {
                    let tx = MetaTx::new(
                        nonce2
                        , TxCaller::try_from(&StandardObject::People(device2.clone())).unwrap()
                        , 0
                        , 0
                        , 0
                        , None
                        , MetaTxBody::NFTBid(NFTBidTx {
                            nft_id: nft_list.desc().calculate_id(),
                            price: 1,
                            coin_id: CoinTokenId::Coin(0)
                        })
                        , Vec::new(),
                    ).build();
                    nonce2 += 1;
                    executor.execute(&new, &tx, None).await.unwrap();
                } else if i == 110 {
                    let beneficiary = state.get_beneficiary(&nft_list.desc().calculate_id()).await.unwrap();
                    assert_eq!(beneficiary, id2);
                    let beneficiary = state.get_beneficiary(&nft_list.nft_list()[0].calculate_id()).await.unwrap();
                    assert_eq!(beneficiary, id2);
                    assert_eq!(state.get_balance(&id2, &CoinTokenId::Coin(0)).await.unwrap(), balance2 - 1);
                    assert_eq!(state.get_balance(&id1, &CoinTokenId::Coin(0)).await.unwrap(), balance1 + 1);
                }
                event_manager.run_event(&new).await.unwrap();
                prev = new;
            }
        });
    }
}

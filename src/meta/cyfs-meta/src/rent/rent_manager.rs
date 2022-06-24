use crate::events::event_manager::{EventManagerRef, EventManagerWeakRef};
use crate::state_storage::{DescExtra, NameExtra, StateRef, StateWeakRef};
use crate::executor::context::{ConfigRef, ConfigWeakRef};
use cyfs_base_meta::{EventType, RentParam, Event, NameRentParam, BlockDesc, BlockDescTrait};
use cyfs_base::*;
use std::sync::{Arc, Weak, Mutex};
use crate::helper::{ArcWeakHelper};
use std::future::Future;
use crate::{State, EventResult};
use async_trait::async_trait;
use crate::*;

pub type RentManagerRef = Arc<RentManager>;
pub type RentManagerWeakRef = Weak<RentManager>;

#[async_trait]
pub trait RentEventListener: Send + Sync + 'static {
    async fn call(&self, block_desc: BlockDesc, event: Event, deduction_amount: i64, arrears_rent: i64, arrears_rent_count: i64) -> BuckyResult<()>;
}

#[async_trait]
impl<F, Fut> RentEventListener for F
    where
        F: Send + Sync + 'static + Fn(BlockDesc, Event, i64, i64, i64) -> Fut,
        Fut: Send + 'static + Future<Output = BuckyResult<()>>,
{
    async fn call(&self, block_desc: BlockDesc, event: Event, deduction_amount: i64, arrears_rent: i64, arrears_rent_count: i64) -> BuckyResult<()> {
        let fut = (self)(block_desc, event, deduction_amount, arrears_rent, arrears_rent_count);
        fut.await
    }
}

pub struct RentManager {
    ref_state: StateWeakRef,
    config: ConfigWeakRef,
    event_manager: EventManagerWeakRef,
    listener: Mutex<Vec<Arc<dyn RentEventListener>>>,
}

impl RentManager {
    pub fn new(ref_state: &StateRef, config: &ConfigRef, event_manager: &EventManagerRef) -> RentManagerRef {
        let manager = RentManagerRef::new(RentManager {
            ref_state: StateRef::downgrade(ref_state),
            config: ConfigRef::downgrade(config),
            event_manager: EventManagerRef::downgrade(event_manager),
            listener: Mutex::new(Vec::new())
        });

        let rent_closure_manager = RentManagerRef::downgrade(&manager);
        event_manager.register_listener(EventType::Rent, move |cur_block: BlockDesc, event: Event| {
            let rent_closure_manager = rent_closure_manager.clone();
            Box::pin(async move {
                if let Event::Rent(param) = &event {
                    rent_closure_manager.to_rc()?.on_rent(&cur_block, &event, param).await
                } else {
                    Err(crate::meta_err!(ERROR_INVALID))
                }
            })
        });

        let name_rent_closure_manager = RentManagerRef::downgrade(&manager);
        event_manager.register_listener(EventType::NameRent, move |cur_block: BlockDesc, event: Event| {
            let name_rent_closure_manager = name_rent_closure_manager.clone();
            Box::pin(async move {
                if let Event::NameRent(param) = &event {
                    name_rent_closure_manager.to_rc()?.on_name_rent(&cur_block, &event, param).await
                } else {
                    Err(crate::meta_err!(ERROR_INVALID))
                }
            })
        });

        return manager;
    }

    async fn on_rent(&self, cur_block: &BlockDesc, event: &Event, param: &RentParam) -> BuckyResult<EventResult> {
        let mut state = self.ref_state.to_rc()?.get_desc_extra(&param.id).await?;
        let num = if state.data_len % 1024 == 0 {state.data_len/1024} else {state.data_len/1024 + 1};
        let rent_value = (state.rent_value as i64 * num) as i64;
        let balance = self.ref_state.to_rc()?.get_balance(&param.id, &CoinTokenId::Coin(state.coin_id)).await?;
        if balance >= rent_value {
            self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(state.coin_id), &state.obj_id, rent_value).await?;
            self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(state.coin_id), cur_block.coinbase(), rent_value).await?;
        } else if balance > 0 {
            self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(state.coin_id), &state.obj_id, balance).await?;
            self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(state.coin_id), cur_block.coinbase(), balance).await?;
        }
        let old_other_charge_balance = state.other_charge_balance;
        state.other_charge_balance -= rent_value;
        if state.other_charge_balance < 0 {
            state.other_charge_balance = 0;
        }
        let rent_arrears = rent_value - balance;
        let mut pay = rent_value;
        if rent_arrears > 0 {
            pay = balance;
            state.rent_arrears += rent_arrears;
            state.rent_arrears_count += 1;
            self.ref_state.to_rc()?.add_or_update_desc_extra(&state).await?;
        } else if old_other_charge_balance != state.other_charge_balance {
            self.ref_state.to_rc()?.add_or_update_desc_extra(&state).await?;
        }
        let listeners = self.get_listeners();
        for listener in listeners.iter() {
            listener.call(cur_block.clone(),
                                    event.clone(),
                                    pay,
                                    state.rent_arrears,
                                    state.rent_arrears_count).await?;
        }
        Ok(EventResult::new(0, Vec::new()))
    }

    async fn on_name_rent(&self, cur_block: &BlockDesc, event: &Event, param: &NameRentParam) -> BuckyResult<EventResult> {
        let mut state = self.ref_state.to_rc()?.get_name_extra(param.name_id.as_str()).await?;

        let coin_id = state.coin_id;
        let balance = self.ref_state.to_rc()?.get_balance(&state.owner, &CoinTokenId::Coin(coin_id)).await?;
        let mut pay = state.rent_value;
        if balance < state.rent_value {
            pay = balance;
            state.rent_arrears_count += 1;
        }
        let old_rent_arrears = state.rent_arrears;
        if balance >= state.rent_value + state.rent_arrears {
            self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(coin_id), &state.owner, state.rent_value + state.rent_arrears).await?;
            self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(coin_id), cur_block.coinbase(), state.rent_value + state.rent_arrears).await?;
            state.rent_arrears = 0;
            state.rent_arrears_count = 0;
        } else if balance > 0 {
            self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(coin_id), &state.owner, balance).await?;
            self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(coin_id), cur_block.coinbase(), balance).await?;
            state.rent_arrears = state.rent_arrears + state.rent_value - balance;
        } else {
            state.rent_arrears += state.rent_value;
        }
        if old_rent_arrears != state.rent_arrears {
            self.ref_state.to_rc()?.add_or_update_name_extra(&state).await?;
        }
        let listeners = self.get_listeners();
        for listener in listeners.iter() {
            listener.call(cur_block.clone(),
                          event.clone(),
                          pay,
                          state.rent_arrears,
                          state.rent_arrears_count).await?;
        }
        Ok(EventResult::new(0, Vec::new()))
    }

    pub fn register_listener(&self, listener: impl RentEventListener) {
        let mut listener_ref = self.listener.lock().unwrap();
        listener_ref.push(Arc::new(listener));
    }

    fn get_listeners(&self) -> Vec<Arc<dyn RentEventListener>> {
        let listener_list = self.listener.lock().unwrap();
        let mut new_list = Vec::new();
        for listener in listener_list.iter() {
            new_list.push(listener.clone());
        }
        new_list
    }

    pub async fn add_rent_desc(&self, cur_block: &BlockDesc, objid: &ObjectId, coin_id: u8, price: i64, data_len: i64) -> BuckyResult<()> {
        let rent_param = RentParam{
            id: objid.clone()
        };
        if price > 0 {
            self.event_manager.to_rc()?.add_or_update_cycle_event(objid.to_string().as_str(), &Event::Rent(rent_param)
                                                         , self.config.to_rc()?.get_rent_cycle(), cur_block.number() + self.config.to_rc()?.get_rent_cycle()).await?;
        }
        self.ref_state.to_rc()?.add_or_update_desc_rent_state(&DescExtra::new_desc_rent_state(objid, 0, 0, price, coin_id, data_len)).await
    }

    pub async fn is_desc_arrears(&self, objid: &ObjectId) -> BuckyResult<bool> {
        let rent_state = self.ref_state.to_rc()?.get_desc_extra(objid).await?;
        if rent_state.rent_arrears > 0 {
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub async fn update_rent_desc(&self, objid: &ObjectId, coin_id: u8, price: i64, data_len: i64) -> BuckyResult<()> {
        self.ref_state.to_rc()?.add_or_update_desc_rent_state(&DescExtra::new_desc_rent_state(objid, 0, 0, price, coin_id, data_len)).await
    }

    // 返回扣了多少租金
    pub async fn check_and_deduct_rent_arrears_for_desc(&self, cur_block: &BlockDesc, account: &ObjectId, ctid: &CoinTokenId) -> BuckyResult<i64> {
        let mut dec_amount = 0;
        if let Ok(mut rent_state) = self.ref_state.to_rc()?.get_desc_extra(account).await {
            if rent_state.rent_arrears > 0 {
                if let CoinTokenId::Coin(coin_id) = ctid {
                    if *coin_id == rent_state.coin_id {
                        let amount = self.ref_state.to_rc()?.get_balance(account, ctid).await?;
                        rent_state.other_charge_balance -= rent_state.rent_arrears;
                        if rent_state.other_charge_balance < 0 {
                            rent_state.other_charge_balance = 0;
                        }
                        // 取租金和账户余额比较小的金额做这次的扣除金额
                        dec_amount = std::cmp::min(amount, rent_state.rent_arrears);
                        self.ref_state.to_rc()?.dec_balance(ctid, account, rent_state.rent_arrears).await?;
                        self.ref_state.to_rc()?.inc_balance(ctid, cur_block.coinbase(), rent_state.rent_arrears).await?;
                        rent_state.rent_arrears -= dec_amount;

                        if rent_state.rent_arrears == 0 {
                            rent_state.rent_arrears_count = 0;
                        }

                        self.ref_state.to_rc()?.add_or_update_desc_extra(&rent_state).await?;
                    }
                }
            }
        }
        Ok(dec_amount)
    }

    pub async fn check_and_charge_for_desc(&self, _objid: &ObjectId, _caller: &ObjectId, _coin_id: u8, _price: i64) -> BuckyResult<()> {
        // if let Ok(mut rent_state) = self.ref_state.to_rc()?.get_desc_extra(_objid) {
        //     if rent_state.coin_id == _coin_id {
        //         rent_state.other_charge_balance += _price;
        //     }
        // }
        Ok(())
    }

    pub async fn delete_rent_desc(&self, obj_id: &ObjectId) -> BuckyResult<()> {
        self.event_manager.to_rc()?.drop_cycle_event(obj_id.to_string().as_str(), self.config.to_rc()?.get_rent_cycle()).await?;
        self.ref_state.to_rc()?.drop_desc_extra(&obj_id).await?;
        Ok(())
    }

    pub async fn add_rent_name(&self, cur_block: &BlockDesc, name: &str, owner: &ObjectId, coin_id: u8, price: i64) -> BuckyResult<()> {
        self.ref_state.to_rc()?.add_or_update_name_rent_state(&NameExtra {
            name_id: name.to_string(),
            rent_arrears: 0,
            rent_arrears_count: 0,
            rent_value: price,
            coin_id,
            owner: owner.clone(),
            buy_coin_id: 0,
            buy_price: 0
        }).await?;

        if price > 0 {
            let event_key = format!("rent_{}", &name);
            self.event_manager.to_rc()?.add_or_update_cycle_event(&event_key, &Event::NameRent(NameRentParam {name_id: name.to_string()})
                                                    , self.config.to_rc()?.get_rent_cycle(), cur_block.number() + self.config.to_rc()?.get_rent_cycle()).await?;
        }

        Ok(())
    }

    pub async fn update_rent_name(&self, cur_block: &BlockDesc, name: &str, owner: &ObjectId, coin_id: u8, price: i64) -> BuckyResult<()> {
        self.add_rent_name(cur_block, name, owner, coin_id, price).await
    }

    pub async fn stop_rent_name(&self, name: &str) -> BuckyResult<()> {
        let event_key = format!("rent_{}", &name);
        self.ref_state.to_rc()?.drop_cycle_event(event_key.as_str(), self.config.to_rc()?.get_rent_cycle()).await
    }

    pub async fn check_and_deduct_rent_arrears_for_name(&self, cur_block: &BlockDesc, name: &str) -> BuckyResult<NameExtra> {
        let mut rent_state= self.ref_state.to_rc()?.get_name_extra(name).await?;
        if rent_state.rent_arrears > 0 {
            let balance = self.ref_state.to_rc()?.get_balance(&rent_state.owner, &CoinTokenId::Coin(rent_state.coin_id)).await?;
            if balance >= rent_state.rent_arrears {
                self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(rent_state.coin_id), &rent_state.owner, rent_state.rent_arrears).await?;
                self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(rent_state.coin_id), &cur_block.coinbase(), rent_state.rent_arrears).await?;
                rent_state.rent_arrears = 0;
                rent_state.rent_arrears_count = 0;
            } else {
                self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(rent_state.coin_id), &rent_state.owner, balance).await?;
                self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(rent_state.coin_id), &cur_block.coinbase(), balance).await?;
                rent_state.rent_arrears -= balance;
            }

            self.ref_state.to_rc()?.add_or_update_name_extra(&rent_state).await?;
        }
        Ok(rent_state)
    }
}

#[cfg(test)]
mod rent_manager_tests {
    use crate::events::event_manager::EventManager;
    use crate::rent::rent_manager::RentManager;
    use cyfs_base_meta::{BlockDesc, BlockDescContent};
    use cyfs_base::{ObjectId, NameInfo, NameRecord, NameLink, CoinTokenId};
    use crate::executor::context::Config;
    use std::collections::HashMap;
    use std::str::FromStr;
    use crate::{sql_storage_tests, State};

    #[test]
    fn test_rent() {
        async_std::task::block_on(async {
            let state = sql_storage_tests::create_state().await;
            let config = Config::new(&state).unwrap();
            let ret = state.create_cycle_event_table(config.get_rent_cycle()).await;
            assert!(ret.is_ok());

            let event_manager = EventManager::new(&state, &config);
            let rent_manager = RentManager::new(&state, &config, &event_manager);

            let id1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();
            let id2 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap();
            let coin_token_id = CoinTokenId::Coin(0);
            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn6").unwrap();
            let amount1: i64 = 3000;
            let amount2: i64 = 3000;
            let id1_rent: i64 = 100;
            let id2_rent: i64 = 110;
            let name_rent: i64 = 130;
            state.inc_balance(&coin_token_id, &id1, amount1).await.unwrap();
            state.inc_balance(&coin_token_id, &id2, amount2).await.unwrap();

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: Some(id1.clone())
            }).await;
            assert!(ret.is_ok());

            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..100 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 33 {
                    let ret = rent_manager.add_rent_desc(&new, &id1, 0, id1_rent, 3459).await;
                    assert!(ret.is_ok());
                } else if i == 38 {
                    let ret = rent_manager.add_rent_desc(&new, &id2, 0, id2_rent, 1203).await;
                    assert!(ret.is_ok());
                } else if i == 67 {
                    let ret = rent_manager.add_rent_name(&new, "test", &id1, 0, name_rent).await;
                    assert!(ret.is_ok());
                }
                let ret = event_manager.run_event(&new).await;
                assert!(ret.is_ok());
                prev = new;
            }

            for _ in 1..501 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                let ret = event_manager.run_event(&new).await;
                assert!(ret.is_ok());
                prev = new;
            }

            let balance = state.get_balance(&id1, &coin_token_id).await.unwrap();
            assert_eq!(balance, amount1 - (id1_rent*4 + name_rent)*5);
            let balance = state.get_balance(&id2, &coin_token_id).await.unwrap();
            assert_eq!(balance, amount2 - 2*id2_rent*5);

            for _ in 1..2501 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                let ret = event_manager.run_event(&new).await;
                assert!(ret.is_ok());
                prev = new;
            }

            let balance = state.get_balance(&baseid1, &coin_token_id).await.unwrap();
            assert_eq!(balance, amount1 + amount2);

            let balance = state.get_balance(&id1, &coin_token_id).await.unwrap();
            assert_eq!(balance, 0);
            let ret = rent_manager.is_desc_arrears(&id1).await;
            assert!(ret.is_ok());
            assert!(ret.unwrap());
            let balance = state.get_balance(&id2, &coin_token_id).await.unwrap();
            assert_eq!(balance, 0);
            let ret = rent_manager.is_desc_arrears(&id2).await;
            assert!(ret.is_ok());
            assert!(ret.unwrap());

            let desc_extra = state.get_desc_extra(&id1).await.unwrap();
            let name_extra = state.get_name_extra("test").await.unwrap();
            assert_eq!(desc_extra.rent_arrears + name_extra.rent_arrears, (id1_rent*4 + name_rent)*30 - amount1);

            let desc_extra = state.get_desc_extra(&id2).await.unwrap();
            assert_eq!(desc_extra.rent_arrears, 2*id2_rent*30 - amount2);

            state.inc_balance(&coin_token_id, &id1, 30000).await.unwrap();
            state.inc_balance(&coin_token_id, &id2, 30000).await.unwrap();

            let ret = rent_manager.check_and_deduct_rent_arrears_for_desc(&prev, &id1, &coin_token_id).await;
            assert!(ret.is_ok());

            let ret = rent_manager.check_and_deduct_rent_arrears_for_desc(&prev, &id2, &coin_token_id).await;
            assert!(ret.is_ok());

            let ret = rent_manager.check_and_deduct_rent_arrears_for_name(&prev, "test").await;
            assert!(ret.is_ok());

            let balance = state.get_balance(&baseid1, &coin_token_id).await.unwrap();
            assert_eq!(balance, 2*id2_rent*30+(id1_rent*4 + name_rent)*30);

        });
    }
}

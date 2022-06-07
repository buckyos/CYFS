use crate::state_storage::{StateRef, StateWeakRef, NameExtra};
use cyfs_base::*;
use crate::executor::context::{ConfigRef, ConfigWeakRef};
use cyfs_base_meta::{Event, BidName, BlockDesc, EventType, StopAuctionParam, NameRentParam, BlockDescTrait};
use crate::rent::rent_manager::{RentManagerRef, RentManagerWeakRef};
use crate::events::event_manager::{EventManagerRef, EventManagerWeakRef};
use std::sync::{Arc, Weak};
use crate::helper::{ArcWeakHelper};
use std::i64::MAX;
use crate::{State, EventResult};
use crate::*;
use cyfs_base::BuckyErrorCode;

pub type AuctionRef = Arc<Auction>;
pub type AuctionWeakRef = Weak<Auction>;

pub struct Auction {
    ref_state: StateWeakRef,
    config: ConfigWeakRef,
    rent_manager: RentManagerWeakRef,
    event_manager: EventManagerWeakRef
}

impl Auction {
    pub fn new(state: &StateRef, config: &ConfigRef, rent_manager: &RentManagerRef, event_manager: &EventManagerRef) -> AuctionRef {
        let auction = AuctionRef::new(Auction {
            ref_state: StateRef::downgrade(state),
            config: ConfigRef::downgrade(config),
            rent_manager: RentManagerRef::downgrade(rent_manager),
            event_manager: EventManagerRef::downgrade(event_manager)
        });

        let auction_ref = AuctionRef::downgrade(&auction);
        event_manager.register_listener(EventType::BidName, move |cur_block: BlockDesc, event: Event| {
            let auction_ref = auction_ref.clone();
            Box::pin(async move {
                if let Event::BidName(param) = &event {
                    auction_ref.to_rc()?.on_bid_name_event(&cur_block, &param).await
                } else {
                    Err(crate::meta_err!(ERROR_INVALID))
                }
            })
        });

        let auction_ref = AuctionRef::downgrade(&auction);
        event_manager.register_listener(EventType::StopAuction, move |cur_block: BlockDesc, event: Event| {
            let auction_ref = auction_ref.clone();
            Box::pin(async move {
                if let Event::StopAuction(param) = &event {
                    auction_ref.to_rc()?.on_stop_auction_event(&cur_block, &param).await
                } else {
                    Err(crate::meta_err!(ERROR_INVALID))
                }
            })
        });

        let auction_ref = AuctionRef::downgrade(&auction);
        rent_manager.register_listener(move |cur_block, event, deduction_amount, arrears_rent, arrears_rent_count| {
            let auction_ref = auction_ref.clone();
            Box::pin(async move {
                match event {
                    Event::NameRent(param) => {
                        auction_ref.to_rc()?.on_name_rent(&cur_block, &param, deduction_amount, arrears_rent, arrears_rent_count).await?;
                    }
                    _ => {}
                }
                Ok(())
            })
        });

        auction
    }

    async fn on_name_rent(&self, cur_block: &BlockDesc, param: &NameRentParam, _: i64, _: i64, arrears_rent_count: i64) -> BuckyResult<EventResult> {
        if arrears_rent_count > 0 && arrears_rent_count < self.config.to_rc()?.name_rent_arrears_auctioned_interval() as i64 {
            let mut name_state = self.ref_state.to_rc()?.get_name_state(param.name_id.as_str()).await?;
            if name_state == NameState::Normal {
                name_state = NameState::Lock;
                self.ref_state.to_rc()?.update_name_state(param.name_id.as_str(), name_state).await?;
            } else if name_state == NameState::Lock {
                return Ok(EventResult::new(0, Vec::new()));
            } else {
                return Err(crate::meta_err!(ERROR_NAME_STATE_ERROR));
            }
        }
        else if arrears_rent_count >= self.config.to_rc()?.name_rent_arrears_auctioned_interval() as i64 {
            self.arrears_auction_name(cur_block, param.name_id.as_str()).await?;
        }
        Ok(EventResult::new(0, Vec::new()))
    }

    fn get_event_key(&self, name: &str) -> String {
        "#bid#_".to_owned() + name
    }

    fn get_stop_event_key(&self, name: &str) -> String {
        "#stop#_".to_owned() + name
    }

    pub async fn bid_name(&self, owner_block: &BlockDesc, name: &str, bid_id: &ObjectId, coin_id: u8, price: i64, rent_price: i64) -> BuckyResult<EventResult> {
        let mut name_state = self.ref_state.to_rc()?.get_name_state(name).await?;
        if name_state != NameState::Auction && name_state != NameState::ActiveAuction && name_state != NameState::ArrearsAuction {
            return Err(crate::meta_err!(ERROR_NAME_STATE_ERROR));
        }

        let mut first_bid_take_effect_block = owner_block.number() + self.config.to_rc()?.max_auction_stop_interval();

        if name_state == NameState::ActiveAuction {
            let stop_event_name = self.get_stop_event_key(name);
            if let Event::StopAuction(param) = self.event_manager.to_rc()?.get_once_event(stop_event_name.as_str()).await? {
                if param.starting_price > price {
                    return Err(crate::meta_err!(ERROR_BID_PRICE_TOO_LOW))
                }

                self.event_manager.to_rc()?.drop_once_event(stop_event_name.as_str()).await?;
                if param.stop_block != MAX {
                    first_bid_take_effect_block = param.stop_block;
                }
            }

            name_state = NameState::Auction;
            self.ref_state.to_rc()?.update_name_state(name, name_state).await?;
        } else if name_state == NameState::ArrearsAuction {
            let stop_event_name = self.get_stop_event_key(name);
            let event_ret = self.event_manager.to_rc()?.get_once_event(stop_event_name.as_str()).await;
            if event_ret.is_ok() {
                if let Event::StopAuction(param) = event_ret.unwrap() {
                    if param.starting_price + self.config.to_rc()?.min_auction_price() > price {
                        return Err(crate::meta_err!(ERROR_BID_PRICE_TOO_LOW))
                    }

                    self.event_manager.to_rc()?.drop_once_event(stop_event_name.as_str()).await?;
                }
            }
        }

        let event_key = self.get_event_key(name);
        match self.event_manager.to_rc()?.get_once_event(event_key.as_str()).await {
            Ok(event) => {
                if let Event::BidName(param) = event {
                    if param.coin_id != coin_id {
                        return Err(crate::meta_err!(ERROR_BID_COIN_NOT_MATCH));
                    }

                    if param.price >= price {
                        return Err(crate::meta_err!(ERROR_BID_PRICE_TOO_LOW));
                    }

                    if price - param.price < self.config.to_rc()?.min_auction_price() {
                        return Err(crate::meta_err!(ERROR_BID_PRICE_TOO_LOW));
                    }

                    if param.bid_id == *bid_id {
                        return Err(crate::meta_err!(ERROR_HAS_BID));
                    }

                    self.event_manager.to_rc()?.drop_once_event(event_key.as_str()).await?;
                    self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(param.coin_id), &param.bid_id, param.price).await?;
                    let balance = self.ref_state.to_rc()?.get_balance(bid_id, &CoinTokenId::Coin(coin_id)).await?;
                    if balance < price {
                        return Err(crate::meta_err!(ERROR_NO_ENOUGH_BALANCE));
                    }
                    self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(coin_id), bid_id, price).await?;

                    let take_effect_block = if param.take_effect_block - owner_block.number() < self.config.to_rc()?.min_auction_stop_interval() {
                        owner_block.number() + self.config.to_rc()?.min_auction_stop_interval()
                    } else {
                        param.take_effect_block
                    };

                    let new_param = BidName {
                        name: name.to_string(),
                        price,
                        coin_id,
                        bid_id: bid_id.clone(),
                        take_effect_block,
                        rent_price,
                    };

                    self.event_manager.to_rc()?.add_or_update_once_event(event_key.as_str(), &Event::BidName(new_param), take_effect_block).await?;
                }

            },
            Err(err) => {
                if let BuckyErrorCode::MetaError(e) = err.code() {
                    if e != ERROR_NOT_FOUND {
                        return Err(crate::meta_err2!(e, err.msg()));
                    }

                    if e == ERROR_NOT_FOUND && name_state != NameState::Auction && name_state != NameState::ArrearsAuction {
                        return Err(crate::meta_err!(ERROR_EXCEPTION));
                    }

                } else {
                    return Err(err);
                }
                if price < self.config.to_rc()?.min_auction_price() {
                    return Err(crate::meta_err!(ERROR_BID_PRICE_TOO_LOW));
                }

                let balance = self.ref_state.to_rc()?.get_balance(bid_id, &CoinTokenId::Coin(coin_id)).await?;
                if balance < price {
                    return Err(crate::meta_err!(ERROR_NO_ENOUGH_BALANCE));
                }
                self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(coin_id), bid_id, price).await?;

                let take_effect_block = if first_bid_take_effect_block - owner_block.number() < self.config.to_rc()?.max_auction_stop_interval() {
                    owner_block.number() + self.config.to_rc()?.min_auction_stop_interval()
                } else {
                    first_bid_take_effect_block
                };

                let new_param = BidName {
                    name: name.to_string(),
                    price,
                    coin_id,
                    bid_id: bid_id.clone(),
                    take_effect_block,
                    rent_price,
                };

                self.ref_state.to_rc()?.add_or_update_once_event(event_key.as_str(), &Event::BidName(new_param), take_effect_block).await?;
            }
        }

        Ok(EventResult::new(0, Vec::new()))
    }

    pub async fn buy_back_name(&self, cur_block: &BlockDesc, name: &str, bid_id: &ObjectId) -> BuckyResult<()> {
        if let Some((name_info, mut name_state)) = self.ref_state.to_rc()?.get_name_info(name).await? {
            if name_state != NameState::ArrearsAuctionWait {
                return Err(crate::meta_err!(ERROR_NAME_STATE_ERROR));
            }

            if name_info.owner.is_none() {
                return Err(crate::meta_err!(ERROR_NAME_STATE_ERROR));
            }

            if name_info.owner.unwrap() != *bid_id {
                return Err(crate::meta_err!(ERROR_BID_NO_AUTH))
            }

            let event_key = self.get_event_key(name);
            match self.event_manager.to_rc()?.get_once_event(event_key.as_str()).await {
                Ok(event) => {
                    if let Event::BidName(param) = event {
                        let balance = self.ref_state.to_rc()?.get_balance(bid_id, &CoinTokenId::Coin(param.coin_id)).await?;
                        if balance < param.price {
                            return Err(crate::meta_err!(ERROR_NO_ENOUGH_BALANCE));
                        }

                        self.event_manager.to_rc()?.drop_once_event(event_key.as_str()).await?;
                        self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(param.coin_id), &param.bid_id, param.price).await?;

                        name_state = NameState::Normal;
                        self.ref_state.to_rc()?.update_name_state(name, name_state).await?;
                        self.rent_manager.to_rc()?.check_and_deduct_rent_arrears_for_name(cur_block, name).await?;

                        let name_rent_state = self.ref_state.to_rc()?.get_name_extra(name).await?;
                        self.rent_manager.to_rc()?.add_rent_name(cur_block, name, &name_rent_state.owner, name_rent_state.coin_id, name_rent_state.rent_value).await?;
                    }
                },
                Err(_) => {
                    return Err(crate::meta_err!(ERROR_EXCEPTION));
                }
            }
            Ok(())
        } else {
            Err(crate::meta_err!(ERROR_EXCEPTION))
        }
    }

    pub async fn active_auction_name(&self, name: &str, stop_block: i64, starting_price: i64) -> BuckyResult<()> {
        let mut name_state = self.ref_state.to_rc()?.get_name_state(name).await?;
        if name_state != NameState::Normal {
            return Err(crate::meta_err!(ERROR_NAME_STATE_ERROR));
        }

        name_state = NameState::ActiveAuction;
        self.ref_state.to_rc()?.update_name_state(name, name_state).await?;
        self.rent_manager.to_rc()?.stop_rent_name(name).await?;

        let param = StopAuctionParam {
            name: name.to_owned(),
            stop_block,
            starting_price
        };

        self.event_manager.to_rc()?.add_or_update_once_event(self.get_stop_event_key(name).as_str(), &Event::StopAuction(param), stop_block).await
    }

    async fn arrears_auction_name(&self, cur_block: &BlockDesc, name: &str) -> BuckyResult<()> {
        let mut name_state = self.ref_state.to_rc()?.get_name_state(name).await?;
        if name_state != NameState::Lock {
            return Err(crate::meta_err!(ERROR_NAME_STATE_ERROR));
        }

        name_state = NameState::ArrearsAuction;
        self.ref_state.to_rc()?.update_name_state(name, name_state).await?;

        let name_extra = self.ref_state.to_rc()?.get_name_extra(name).await?;
        let stop_block = cur_block.number() + self.config.to_rc()?.max_auction_stop_interval();
        let param = StopAuctionParam {
            name: name.to_owned(),
            stop_block,
            starting_price: name_extra.buy_price,
        };
        self.event_manager.to_rc()?.add_or_update_once_event(self.get_stop_event_key(name).as_str(), &Event::StopAuction(param), stop_block).await?;
        self.rent_manager.to_rc()?.stop_rent_name(name).await
    }

    pub async fn cancel_auction(&self, cur_block: &BlockDesc, name: &str) -> BuckyResult<()> {
        let mut name_state = self.ref_state.to_rc()?.get_name_state(name).await?;
        if name_state != NameState::ActiveAuction {
            return Err(crate::meta_err!(ERROR_NAME_STATE_ERROR));
        }

        name_state = NameState::Normal;
        self.ref_state.to_rc()?.update_name_state(name, name_state).await?;
        let name_rent_state = self.ref_state.to_rc()?.get_name_extra(name).await?;
        self.rent_manager.to_rc()?.add_rent_name(cur_block, name, &name_rent_state.owner, name_rent_state.coin_id, name_rent_state.rent_value).await?;
        self.event_manager.to_rc()?.drop_once_event(self.get_stop_event_key(name).as_str()).await
    }

    async fn on_stop_auction_event(&self, _: &BlockDesc, event: &StopAuctionParam) -> BuckyResult<EventResult> {
        if let Some((_, mut name_state)) = self.ref_state.to_rc()?.get_name_info(event.name.as_str()).await? {
            if name_state == NameState::ActiveAuction {
                name_state = NameState::Normal;
                self.ref_state.to_rc()?.update_name_state(event.name.as_str(), name_state).await?;
            } else if name_state == NameState::ArrearsAuction {
                let param = StopAuctionParam {
                    name: event.name.clone(),
                    stop_block: MAX,
                    starting_price: self.config.to_rc()?.min_auction_price()
                };
                self.event_manager.to_rc()?.add_or_update_once_event(self.get_stop_event_key(event.name.as_str()).as_str()
                                                                     , &Event::StopAuction(param), MAX).await?;
            }
        }
        Ok(EventResult::new(0, Vec::new()))
    }

    async fn on_bid_name_event(&self, cur_block: &BlockDesc, event: &BidName) -> BuckyResult<EventResult> {
        if let Some((mut name_info, mut name_state)) = self.ref_state.to_rc()?.get_name_info(event.name.as_str()).await? {
            if name_state == NameState::ArrearsAuction {
                name_state = NameState::ArrearsAuctionWait;
                self.ref_state.to_rc()?.update_name_state(event.name.as_str(), name_state).await?;
                let mut new_event = event.clone();
                let take_effect_block = cur_block.number() + self.config.to_rc()?.arrears_auction_wait_interval();
                new_event.take_effect_block = take_effect_block;
                let event_key = self.get_event_key(event.name.as_str());
                self.event_manager.to_rc()?.add_or_update_once_event(event_key.as_str(), &Event::BidName(new_event), take_effect_block).await?;
            } else if name_state == NameState::ArrearsAuctionWait {
                name_state = NameState::Normal;
                self.ref_state.to_rc()?.update_name_state(event.name.as_str(), name_state).await?;
                self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(event.coin_id), &name_info.owner.unwrap(), event.price).await?;

                self.rent_manager.to_rc()?.check_and_deduct_rent_arrears_for_name(cur_block, event.name.as_str()).await?;

                name_info.owner = Some(event.bid_id);
                name_info.record = NameRecord {
                    link: NameLink::ObjectLink(event.bid_id.clone()),
                    user_data: "".to_string()
                };
                self.ref_state.to_rc()?.update_name_info(event.name.as_str(), &name_info).await?;

                self.rent_manager.to_rc()?.update_rent_name(cur_block, event.name.as_str(), &event.bid_id, event.coin_id, event.rent_price).await?;
                self.ref_state.to_rc()?.add_or_update_name_buy_price(&NameExtra::new_buy_price(event.name.as_str(), &event.bid_id
                                                                                               , event.coin_id, event.price)).await?;
            } else if name_state == NameState::Auction {
                name_state = NameState::Normal;
                self.ref_state.to_rc()?.update_name_state(event.name.as_str(), name_state).await?;

                if name_info.owner.is_none() {
                    self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(event.coin_id), cur_block.coinbase(), event.price).await?;
                } else {
                    self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(event.coin_id), &name_info.owner.unwrap(), event.price).await?;
                    self.rent_manager.to_rc()?.check_and_deduct_rent_arrears_for_name(cur_block, event.name.as_str()).await?;
                }
                name_info.owner = Some(event.bid_id);
                name_info.record = NameRecord {
                    link: NameLink::ObjectLink(event.bid_id.clone()),
                    user_data: "".to_string()
                };
                self.ref_state.to_rc()?.update_name_info(event.name.as_str(), &name_info).await?;
                self.rent_manager.to_rc()?.update_rent_name(cur_block, event.name.as_str(), &event.bid_id, event.coin_id, event.rent_price).await?;
                self.ref_state.to_rc()?.add_or_update_name_buy_price(&NameExtra::new_buy_price(event.name.as_str(), &event.bid_id
                                                                                               , event.coin_id, event.price)).await?;
            }
            Ok(EventResult::new(0, Vec::new()))
        } else {
            Err(crate::meta_err!(ERROR_EXCEPTION))
        }
    }
}

#[cfg(test)]
mod auction_tests {
    use crate::executor::context::Config;
    use crate::events::event_manager::EventManager;
    use crate::rent::rent_manager::RentManager;
    use crate::name_auction::auction::Auction;
    use cyfs_base::{ObjectId, NameInfo, NameRecord, NameLink, NameState, CoinTokenId};
    use cyfs_base_meta::{BlockDesc, BlockDescContent, ERROR_ALREADY_EXIST};
    use std::collections::HashMap;
    use std::i64::MAX;
    use std::str::FromStr;
    use crate::helper::get_meta_err_code;
    use crate::{sql_storage_tests, State};

    #[test]
    fn test_bidname() {
        async_std::task::block_on(async {
            let state = sql_storage_tests::create_state().await;
            let config = Config::new(&state).unwrap();
            let ret = state.create_cycle_event_table(config.get_rent_cycle()).await;
            assert!(ret.is_ok());

            let event_manager = EventManager::new(&state, &config);
            let rent_manager = RentManager::new(&state, &config, &event_manager);
            let auction = Auction::new(&state, &config, &rent_manager, &event_manager);

            let id1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();
            let id2 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap();
            let coin_token_id = CoinTokenId::Coin(0);
            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn6").unwrap();

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(!ret.is_ok());
            assert_eq!(get_meta_err_code(ret.as_ref().err().unwrap()).unwrap(), ERROR_ALREADY_EXIST);

            let name = "test";
            assert!(state.inc_balance(&coin_token_id, &id2, 3000).await.is_ok());

            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..100 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 67 {
                    let ret = auction.bid_name(&new, name, &id1, 0, 100, 100).await;
                    assert!(!ret.is_ok());
                    assert!(state.inc_balance(&coin_token_id, &id1, 3000).await.is_ok());
                    let ret = auction.bid_name(&new, name, &id1, 0, 100, 100).await;
                    assert!(ret.is_ok());
                    assert_eq!(state.get_balance(&id1, &coin_token_id).await.unwrap(), 2900);
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Auction);
                    assert!(ret.0.owner.is_none());
                } else if i == 69 {
                    let ret = auction.bid_name(&new, name, &id2, 0, 100, 100).await;
                    assert!(!ret.is_ok());

                    let ret = auction.bid_name(&new, name, &id2, 0, 150, 150).await;
                    assert!(ret.is_ok());
                    assert_eq!(state.get_balance(&id1, &coin_token_id).await.unwrap(), 3000);
                    assert_eq!(state.get_balance(&id2, &coin_token_id).await.unwrap(), 2850);
                } else if i == 71 {
                    let ret = auction.bid_name(&new, name, &id2, 0, 200, 200).await;
                    assert!(!ret.is_ok());
                }
                let ret = event_manager.run_event(&new).await;
                assert!(ret.is_ok());
                if i == 69 + config.max_auction_stop_interval() {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                    assert!(ret.0.owner.is_some());
                    assert_eq!(ret.0.owner.unwrap(), id2);
                }
                prev = new;
            }

            for _ in 1..501 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                let ret = event_manager.run_event(&new).await;
                assert!(ret.is_ok());
                prev = new;
            }

            let ret = auction.bid_name(&prev, name, &id1, 0, 100, 100).await;
            assert!(!ret.is_ok());

            let ret = state.get_name_info(name).await;
            assert!(ret.is_ok());
            let ret = ret.unwrap();
            assert!(ret.is_some());
            let ret = ret.unwrap();
            assert_eq!(ret.0.owner.unwrap(), id2);
            let balance = state.get_balance(&id2, &coin_token_id).await.unwrap();
            assert_eq!(balance, 2100);
            let balance = state.get_balance(&baseid1, &coin_token_id).await.unwrap();
            assert_eq!(balance, 900);

        });
    }

    #[test]
    fn test_active_auction() {
        async_std::task::block_on(async {
            let state = sql_storage_tests::create_state().await;
            let config = Config::new(&state).unwrap();
            let ret = state.create_cycle_event_table(config.get_rent_cycle()).await;
            assert!(ret.is_ok());

            let event_manager = EventManager::new(&state, &config);
            let rent_manager = RentManager::new(&state, &config, &event_manager);
            let auction = Auction::new(&state, &config, &rent_manager, &event_manager);

            let id1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();
            let id2 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap();
            let coin_token_id = CoinTokenId::Coin(0);
            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn6").unwrap();

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(!ret.is_ok());

            let name = "test";
            assert!(state.inc_balance(&coin_token_id, &id2, 3000).await.is_ok());

            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..601 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 67 {
                    let ret = auction.bid_name(&new, name, &id1, 0, 100, 100).await;
                    assert!(!ret.is_ok());
                    assert!(state.inc_balance(&coin_token_id, &id1, 3000).await.is_ok());
                    let ret = auction.bid_name(&new, name, &id1, 0, 100, 100).await;
                    assert!(ret.is_ok());
                    assert_eq!(state.get_balance(&id1, &coin_token_id).await.unwrap(), 2900);
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Auction);
                    assert!(ret.0.owner.is_none());
                }

                if i == 67 + config.max_auction_stop_interval() + 20 {
                    let ret = auction.active_auction_name(name, 67 + config.max_auction_stop_interval() + 40, 300).await;
                    assert!(ret.is_ok());
                    let ret = event_manager.get_cycle_event(format!("rent_{}", name).as_str(), config.get_rent_cycle()).await;
                    assert!(ret.is_err());
                    let ret = auction.active_auction_name(name, 67 + config.max_auction_stop_interval() + 40, 300).await;
                    assert!(!ret.is_ok());
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::ActiveAuction);
                }

                if i == 67 + config.max_auction_stop_interval() + 50 {
                    let ret = auction.active_auction_name(name, MAX, 300).await;
                    assert!(ret.is_ok());
                    let ret = event_manager.get_cycle_event(format!("rent_{}", name).as_str(), config.get_rent_cycle()).await;
                    assert!(ret.is_err());
                }
                if i == 67 + config.max_auction_stop_interval() + 60 {
                    let ret = auction.cancel_auction(&new, name).await;
                    assert!(ret.is_ok());
                    let ret = event_manager.get_cycle_event(format!("rent_{}", name).as_str(), config.get_rent_cycle()).await;
                    assert!(ret.is_ok());
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                }

                if i == 67 + config.max_auction_stop_interval() + 70 {
                    let ret = auction.bid_name(&new, name, &id2, 0, 300, 300).await;
                    assert!(!ret.is_ok());

                    let ret = auction.active_auction_name(name, MAX, 300).await;
                    assert!(ret.is_ok());
                    let ret = event_manager.get_cycle_event(format!("rent_{}", name).as_str(), config.get_rent_cycle()).await;
                    assert!(ret.is_err());
                }
                if i == 67 + config.max_auction_stop_interval() + 75 {
                    let ret = auction.bid_name(&new, name, &id2, 0, 299, 300).await;
                    assert!(!ret.is_ok());
                    let ret = auction.bid_name(&new, name, &id2, 0, 300, 300).await;
                    assert!(ret.is_ok());
                }

                let ret = event_manager.run_event(&new).await;
                assert!(ret.is_ok());

                if i == 67 + config.max_auction_stop_interval() {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                    assert!(ret.0.owner.is_some());
                    assert_eq!(ret.0.owner.unwrap(), id1);
                }
                if i == 67 + config.max_auction_stop_interval() + 40 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                }

                if i == 67 + config.max_auction_stop_interval() + 75 + config.max_auction_stop_interval() -1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Auction);
                    assert_eq!(ret.0.owner.unwrap(), id1);

                    let balance = state.get_balance(&id2, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 2700);
                    let balance = state.get_balance(&id1, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 2900);
                }

                if i == 67 + config.max_auction_stop_interval() + 75 + config.max_auction_stop_interval() {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                    assert_eq!(ret.0.owner.unwrap(), id2);

                    let balance = state.get_balance(&id2, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 2700);
                    let balance = state.get_balance(&id1, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 3200);
                    let balance = state.get_balance(&baseid1, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 100);
                }
                prev = new;
            }
        });
    }

    #[test]
    fn test_arrears_auction() {
        async_std::task::block_on(async {
            let state = sql_storage_tests::create_state().await;
            let config = Config::new(&state).unwrap();
            let ret = state.create_cycle_event_table(config.get_rent_cycle()).await;
            assert!(ret.is_ok());

            let event_manager = EventManager::new(&state, &config);
            let rent_manager = RentManager::new(&state, &config, &event_manager);
            let auction = Auction::new(&state, &config, &rent_manager, &event_manager);

            let id1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();
            let id2 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap();
            let coin_token_id = CoinTokenId::Coin(0);
            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn6").unwrap();

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(!ret.is_ok());

            let name = "test";
            assert!(state.inc_balance(&coin_token_id, &id2, 3000).await.is_ok());

            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..2601 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 67 {
                    let ret = auction.bid_name(&new, name, &id1, 0, 100, 100).await;
                    assert!(!ret.is_ok());
                    assert!(state.inc_balance(&coin_token_id, &id1, 400).await.is_ok());
                    let ret = auction.bid_name(&new, name, &id1, 0, 100, 100).await;
                    assert!(ret.is_ok());
                    assert_eq!(state.get_balance(&id1, &coin_token_id).await.unwrap(), 300);
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Auction);
                    assert!(ret.0.owner.is_none());
                }

                let ret = event_manager.run_event(&new).await;
                assert!(ret.is_ok());

                if i == 67 + config.max_auction_stop_interval() - 1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Auction);
                    assert!(ret.0.owner.is_none());
                }

                if i == 67 + config.max_auction_stop_interval() {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                    assert!(ret.0.owner.is_some());
                    assert_eq!(ret.0.owner.unwrap(), id1);
                }

                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * 4 - 1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                }

                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * 4 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Lock);
                }

                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) - 1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Lock);
                }
                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::ArrearsAuction);
                }
                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) + config.max_auction_stop_interval() - 1 {
                    let ret = auction.bid_name(&new, name, &id2, 0, 99, 99).await;
                    assert!(!ret.is_ok())
                }
                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) + config.max_auction_stop_interval() {
                    let ret = auction.bid_name(&new, name, &id2, 0, 99, 99).await;
                    assert!(ret.is_ok());
                    let balance = state.get_balance(&id2, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 2901);
                }

                if i == 67 + config.max_auction_stop_interval() * 3 + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) - 1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::ArrearsAuction);
                }
                if i == 67 + config.max_auction_stop_interval() * 3 + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::ArrearsAuctionWait);
                }

                if i == 67 + config.max_auction_stop_interval() * 3 + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) + config.arrears_auction_wait_interval() - 1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::ArrearsAuctionWait);
                }
                if i == 67 + config.max_auction_stop_interval() * 3 + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) + config.arrears_auction_wait_interval() {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                    assert_eq!(ret.0.owner.unwrap(), id2);

                    let balance = state.get_balance(&id2, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 2901);
                    let balance = state.get_balance(&id1, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 0);
                }
                prev = new;
            }

        });
    }

    #[test]
    fn test_arrears_auction_bid_back() {
        async_std::task::block_on(async {
            let state = sql_storage_tests::create_state().await;
            let config = Config::new(&state).unwrap();
            let ret = state.create_cycle_event_table(config.get_rent_cycle()).await;
            assert!(ret.is_ok());

            let event_manager = EventManager::new(&state, &config);
            let rent_manager = RentManager::new(&state, &config, &event_manager);
            let auction = Auction::new(&state, &config, &rent_manager, &event_manager);

            let id1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn4").unwrap();
            let id2 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn5").unwrap();
            let coin_token_id = CoinTokenId::Coin(0);
            let baseid1 = ObjectId::from_str("5r4MYfF7qVAbn1gdNy9JaNQUW5DfFM8yD3pnwFWY8nn6").unwrap();

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(ret.is_ok());

            let ret = state.create_name_info("test", &NameInfo {
                sub_records: HashMap::new(),
                record: NameRecord {
                    link: NameLink::OtherNameLink("OtherNameLink".to_owned()),
                    user_data: "".to_owned()
                },
                owner: None
            }).await;
            assert!(!ret.is_ok());

            let name = "test";
            assert!(state.inc_balance(&coin_token_id, &id2, 3000).await.is_ok());

            let mut prev = BlockDesc::new(BlockDescContent::new(baseid1.clone(), None)).build();
            for i in 1..2601 {
                let new = BlockDesc::new(BlockDescContent::new(baseid1.clone(), Some(&prev))).build();
                if i == 67 {
                    let ret = auction.bid_name(&new, name, &id1, 0, 100, 100).await;
                    assert!(!ret.is_ok());
                    assert!(state.inc_balance(&coin_token_id, &id1, 400).await.is_ok());
                    let ret = auction.bid_name(&new, name, &id1, 0, 100, 100).await;
                    assert!(ret.is_ok());
                    assert_eq!(state.get_balance(&id1, &coin_token_id).await.unwrap(), 300);
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Auction);
                    assert!(ret.0.owner.is_none());
                }

                let ret = event_manager.run_event(&new).await;
                assert!(ret.is_ok());

                if i == 67 + config.max_auction_stop_interval() - 1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Auction);
                    assert!(ret.0.owner.is_none());
                }

                if i == 67 + config.max_auction_stop_interval() {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                    assert!(ret.0.owner.is_some());
                    assert_eq!(ret.0.owner.unwrap(), id1);
                }

                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * 4 - 1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                }

                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * 4 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Lock);
                }

                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) - 1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Lock);
                }
                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::ArrearsAuction);
                }
                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) + config.max_auction_stop_interval() - 1 {
                    let ret = auction.bid_name(&new, name, &id2, 0, 99, 99).await;
                    assert!(!ret.is_ok())
                }
                if i == 67 + config.max_auction_stop_interval() + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) + config.max_auction_stop_interval() {
                    let ret = auction.bid_name(&new, name, &id2, 0, 99, 99).await;
                    assert!(ret.is_ok());
                    let balance = state.get_balance(&id2, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 2901);
                }

                if i == 67 + config.max_auction_stop_interval() * 3 + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) - 1 {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::ArrearsAuction);
                }
                if i == 67 + config.max_auction_stop_interval() * 3 + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) {
                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::ArrearsAuctionWait);
                }

                if i == 67 + config.max_auction_stop_interval() * 3 + config.get_rent_cycle() * (4 + config.name_rent_arrears_auctioned_interval() as i64 - 1) + 1 {
                    let ret = auction.buy_back_name(&new, name, &id1).await;
                    assert!(!ret.is_ok());
                    state.inc_balance(&coin_token_id, &id1, 3000).await.unwrap();

                    let ret = auction.buy_back_name(&new, name, &id1).await;
                    assert!(ret.is_ok());

                    let ret = state.get_name_info(name).await;
                    assert!(ret.is_ok());
                    let ret = ret.unwrap();
                    assert!(ret.is_some());
                    let ret = ret.unwrap();
                    assert!(ret.1 == NameState::Normal);
                    assert_eq!(ret.0.owner.unwrap(), id1);

                    let balance = state.get_balance(&id2, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 3000);
                    let balance = state.get_balance(&id1, &coin_token_id).await.unwrap();
                    assert_eq!(balance, 2500);
                }
                prev = new;
            }

        });
    }
}

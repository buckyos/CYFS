use std::sync::{Arc, Weak};
use cyfs_base::{BuckyResult, CoinTokenId, hash_data, ObjectDesc, ObjectId, RawConvertTo};
use crate::*;
use crate::events::event_manager::{EventManagerRef, EventManagerWeakRef};

pub type NFTAuctionRef = Arc<NFTAuction>;
pub type NFTAuctionWeakRef = Weak<NFTAuction>;

pub struct NFTAuction {
    ref_state: StateWeakRef,
    config: ConfigWeakRef,
    event_manager: EventManagerWeakRef
}

impl NFTAuction {
    pub fn new(state: &StateRef, config: &ConfigRef, event_manager: &EventManagerRef) -> NFTAuctionRef {
        let auction = NFTAuctionRef::new(
            Self {
                ref_state: StateRef::downgrade(state),
                config: ConfigRef::downgrade(config),
                event_manager: EventManagerRef::downgrade(event_manager)
            }
        );

        let auction_ref = NFTAuctionRef::downgrade(&auction);
        event_manager.register_listener(EventType::NFTStopAuction, move |cur_block: BlockDesc, event: Event| {
           let auction_ref = auction_ref.clone();
            Box::pin(async move {
                if let Event::NFTStopAuction(event) = event {
                    auction_ref.to_rc()?.on_stop_auction(&cur_block, &event).await
                } else {
                    Err(crate::meta_err!(ERROR_INVALID))
                }
            })
        });
        auction
    }

    async fn on_stop_auction(&self, cur_block: &BlockDesc, params: &NFTStopAuction) -> BuckyResult<EventResult> {
        log::info!("on_stop_auction block {} nft_id {}", cur_block.number(), params.nft_id.to_string());
        let (nft_desc, _, state) = self.ref_state.to_rc()?.nft_get(&params.nft_id).await?;
        let beneficiary = self.ref_state.to_rc()?.get_beneficiary(&params.nft_id).await?;

        if let NFTState::Auctioning((price, coin_id, stop_block)) = state {
            assert_eq!(stop_block as i64, cur_block.number());
            let bid_list = self.ref_state.to_rc()?.nft_get_bid_list(&params.nft_id, 0, i64::MAX).await?;
            if bid_list.is_empty() {
                self.ref_state.to_rc()?.nft_update_state(&params.nft_id, &NFTState::Normal).await?;
                return Ok(EventResult::new(0, Vec::new()));
            }
            let mut max_price_user = ObjectId::default();
            let mut max_price = 0;
            for (buyer_id, bid_price, pay_coin_id) in bid_list.iter() {
                assert_eq!(&coin_id, pay_coin_id);
                assert!(*bid_price >= price);

                if *bid_price > max_price {
                    max_price_user = buyer_id.clone();
                    max_price = *bid_price;
                }
            }

            for (buyer_id, bid_price, pay_coin_id) in bid_list.iter() {
                if buyer_id != &max_price_user {
                    self.ref_state.to_rc()?.inc_balance(pay_coin_id, buyer_id, *bid_price as i64).await?;
                }
            }

            let balance = self.ref_state.to_rc()?.get_balance(&params.nft_id, &CoinTokenId::Coin(0)).await?;
            if balance > 0 {
                self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(0), &params.nft_id, balance).await?;
                self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(0), &beneficiary, balance).await?;
            }

            self.ref_state.to_rc()?.inc_balance(&coin_id, &beneficiary, max_price as i64).await?;
            self.ref_state.to_rc()?.set_beneficiary(&params.nft_id, &max_price_user).await?;
            self.ref_state.to_rc()?.nft_update_state(&params.nft_id, &NFTState::Normal).await?;

            if let NFTDesc::ListDesc(sub_list) = nft_desc {
                for sub_nft in sub_list.content().nft_list.iter() {
                    let sub_id = sub_nft.calculate_id();
                    let balance = self.ref_state.to_rc()?.get_balance(&sub_id, &CoinTokenId::Coin(0)).await?;
                    if balance > 0 {
                        self.ref_state.to_rc()?.dec_balance(&CoinTokenId::Coin(0), &sub_id, balance).await?;
                        self.ref_state.to_rc()?.inc_balance(&CoinTokenId::Coin(0), &beneficiary, balance).await?;
                    }
                    self.ref_state.to_rc()?.set_beneficiary(&sub_id, &max_price_user).await?;
                }
            }

            Ok(EventResult::new(0, max_price_user.as_slice().to_vec()))
        } else {
            Err(meta_err!(ERROR_NFT_IS_NOT_AUCTIONING))
        }
    }

    fn get_nft_event(nft_id: &ObjectId) -> String {
        format!("nft_{}", nft_id.to_string())
    }

    pub async fn auction(&self, caller_id: &ObjectId, nft_id: &ObjectId, price: u64, coin_id: &CoinTokenId, stop_block: u64) -> BuckyResult<()> {
        let (nft_desc, _, state) = self.ref_state.to_rc()?.nft_get(nft_id).await?;
        if let NFTState::Auctioning(_) = state {
            return Err(meta_err!(ERROR_NFT_IS_AUCTIONING));
        }

        let beneficiary = self.ref_state.to_rc()?.get_beneficiary(nft_id).await?;
        if &beneficiary != caller_id {
            return Err(meta_err!(ERROR_ACCESS_DENIED));
        }

        if let NFTDesc::FileDesc2((_, parent_id)) = nft_desc {
            if parent_id.is_some() {
                let parent_beneficiary = self.ref_state.to_rc()?.get_beneficiary(parent_id.as_ref().unwrap()).await?;
                if beneficiary == parent_beneficiary {
                    return Err(meta_err!(ERROR_NOT_SUPPORT));
                }
                let (_, _, parent_state) = self.ref_state.to_rc()?.nft_get(parent_id.as_ref().unwrap()).await?;
                if let NFTState::Auctioning(_) = parent_state {
                    return Err(meta_err!(ERROR_NFT_IS_AUCTIONING));
                }
            }
        } else if let NFTDesc::ListDesc(sub_list) = nft_desc {
            for sub_nft in sub_list.content().nft_list.iter() {
                let sub_id = sub_nft.calculate_id();
                let sub_beneficiary = self.ref_state.to_rc()?.get_beneficiary(&sub_id).await?;
                if sub_beneficiary != beneficiary {
                    return Err(meta_err!(ERROR_NOT_SUPPORT));
                }
                self.ref_state.to_rc()?.nft_update_state(&sub_id, &NFTState::Normal).await?;

                let apply_list = self.ref_state.to_rc()?.nft_get_apply_buy_list(&sub_id, 0, i64::MAX).await?;
                for (buyer_id, price, coin_id) in apply_list.iter() {
                    let event_params = Event::NFTCancelApplyBuy(NFTCancelApplyBuyParam {
                        nft_id: sub_id.clone(),
                        user_id: buyer_id.clone(),
                    });
                    let event_id = hash_data(event_params.to_vec()?.as_slice());
                    self.event_manager.to_rc()?.drop_once_event(event_id.to_string().as_str()).await?;
                    self.ref_state.to_rc()?.inc_balance(coin_id, buyer_id, *price as i64).await?;
                }
                self.ref_state.to_rc()?.nft_remove_all_apply_buy(&sub_id).await?;
            }
        }
        let state = NFTState::Auctioning((price, coin_id.clone(), stop_block));
        self.ref_state.to_rc()?.nft_update_state(nft_id, &state).await?;

        self.event_manager.to_rc()?.add_or_update_once_event(
            Self::get_nft_event(nft_id).as_str(),
            &Event::NFTStopAuction(NFTStopAuction {
                nft_id: nft_id.clone()
            }), stop_block as i64).await?;

        let apply_list = self.ref_state.to_rc()?.nft_get_apply_buy_list(nft_id, 0, i64::MAX).await?;
        for (buyer_id, price, coin_id) in apply_list.iter() {
            let event_params = Event::NFTCancelApplyBuy(NFTCancelApplyBuyParam {
                nft_id: nft_id.clone(),
                user_id: buyer_id.clone(),
            });
            let event_id = hash_data(event_params.to_vec()?.as_slice());
            self.event_manager.to_rc()?.drop_once_event(event_id.to_string().as_str()).await?;
            self.ref_state.to_rc()?.inc_balance(coin_id, buyer_id, *price as i64).await?;
        }
        self.ref_state.to_rc()?.nft_remove_all_apply_buy(nft_id).await?;

        Ok(())
    }

    pub async fn create_and_auction(
        &self,
        nft_id: &ObjectId,
        desc: &NFTDesc,
        name: &str,
        price: u64,
        coin_id: &CoinTokenId,
        stop_block: u64) -> BuckyResult<()> {
        let state = NFTState::Auctioning((price, coin_id.clone(), stop_block));
        self.ref_state.to_rc()?.nft_create(nft_id, desc, name, &state).await?;

        self.event_manager.to_rc()?.add_or_update_once_event(
            Self::get_nft_event(nft_id).as_str(),
            &Event::NFTStopAuction(NFTStopAuction {
                nft_id: nft_id.clone()
            }),
            stop_block as i64).await?;
        Ok(())
    }

    pub async fn bid(&self, buyer_id: &ObjectId, nft_id: &ObjectId, price: u64, coin_id: &CoinTokenId) -> BuckyResult<()> {
        let (_, _, state) = self.ref_state.to_rc()?.nft_get(nft_id).await?;
        if let NFTState::Auctioning((start_price, need_coin_id, _)) = state {
            if coin_id != &need_coin_id {
                return Err(meta_err!(ERROR_BID_COIN_NOT_MATCH));
            }

            if price < start_price {
                return Err(meta_err!(ERROR_BID_PRICE_TOO_LOW));
            }

            let bid = self.ref_state.to_rc()?.nft_get_bid(nft_id, buyer_id).await?;
            let pay = if bid.is_some() {
                let (latest_price, _) = bid.unwrap();
                if price < latest_price {
                    return Ok(())
                } else {
                    price - latest_price
                }
            } else {
                price
            };

            self.ref_state.to_rc()?.dec_balance(coin_id, buyer_id, pay as i64).await?;
            self.ref_state.to_rc()?.nft_add_bid(nft_id, buyer_id, price, coin_id).await?;

            Ok(())
        } else {
            return Err(meta_err!(ERROR_NFT_IS_NOT_AUCTIONING));
        }
    }
}

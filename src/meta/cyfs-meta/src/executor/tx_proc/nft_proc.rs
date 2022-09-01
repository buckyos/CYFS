use cyfs_base::{BuckyResult, CoinTokenId, hash_data, ObjectDesc, ObjectTypeCode, RawConvertTo};
use cyfs_base_meta::*;
use crate::{ArcWeakHelper, ExecuteContext, get_meta_err_code, meta_err, State};
use crate::tx_executor::TxExecutor;

impl TxExecutor {
    pub async fn execute_nft_create(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTCreateTx) -> BuckyResult<()> {

        let object_id = tx.desc.nft_id();
        if object_id.obj_type_code() != ObjectTypeCode::File {
            return Err(meta_err!(ERROR_NOT_SUPPORT));
        }
        // let beneficiary = if tx.desc.author_id().is_some() {
        //     tx.desc.author_id().as_ref().unwrap().clone()
        // } else {
        //     return Err(meta_err!(ERROR_PARAM_ERROR));
        // };

        let beneficiary = context.caller().id().clone();
        if tx.desc.owner_id().is_none() || &beneficiary != tx.desc.owner_id().as_ref().unwrap() {
            return Err(meta_err!(ERROR_NFT_CREATE_ONLY_OWNER))
        }

        context.ref_state().to_rc()?.set_beneficiary(&object_id, &beneficiary).await?;

        if let NFTState::Auctioning((price, coind_id, stop_block)) = &tx.state {
            self.nft_auction.to_rc()?.create_and_auction(
                &object_id,
                &tx.desc,
                tx.name.as_str(),
                *price,
                coind_id,
                *stop_block + context.block().number() as u64).await?;
        } else if let NFTState::Selling((price, _, stop_block)) = &tx.state {
            log::info!("create and sell {} price {} stop_block {}", object_id.to_string(), price, stop_block);
            context.ref_state().to_rc()?.nft_create(
                &object_id,
                &tx.desc,
                tx.name.as_str(),
                &tx.state).await?;

            let event_params = Event::NFTStopSell(NFTStopSell {
                nft_id: object_id.clone()
            });
            let id = hash_data(event_params.to_vec()?.as_slice());
            self.event_manager.to_rc()?.add_or_update_once_event(id.to_string().as_str(), &event_params, *stop_block as i64 + context.block().number()).await?;
        } else {
            log::info!("create nft {}", object_id.to_string());
            context.ref_state().to_rc()?.nft_create(
                &object_id,
                &tx.desc,
                tx.name.as_str(),
                &tx.state).await?;
        }

        Ok(())
    }

    pub async fn execute_nft_create2(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTCreateTx2
    ) -> BuckyResult<()> {
        let object_id = tx.desc.nft_id();
        let beneficiary = context.caller().id().clone();
        if tx.desc.owner_id().is_none() || &beneficiary != tx.desc.owner_id().as_ref().unwrap() {
            return Err(meta_err!(ERROR_NFT_CREATE_ONLY_OWNER))
        }
        context.ref_state().to_rc()?.set_beneficiary(&object_id, &beneficiary).await?;

        let nft_name = tx.name.as_str();
        let nft_state = &tx.state;

        match &tx.desc {
            NFTDesc::FileDesc(_) => {
                if let NFTState::Auctioning((price, coin_id, stop_block)) = nft_state {
                    self.nft_auction.to_rc()?.create_and_auction(
                        &object_id,
                        &tx.desc,
                        nft_name,
                        *price,
                        coin_id,
                        *stop_block + context.block().number() as u64).await?;
                } else if let NFTState::Selling((_, _, stop_block)) = nft_state {
                    context.ref_state().to_rc()?.nft_create(
                        &object_id,
                        &tx.desc,
                        tx.name.as_str(),
                        &tx.state).await?;

                    let event_params = Event::NFTStopSell(NFTStopSell {
                        nft_id: object_id.clone()
                    });
                    let id = hash_data(event_params.to_vec()?.as_slice());
                    self.event_manager.to_rc()?.add_or_update_once_event(id.to_string().as_str(), &event_params, *stop_block as i64 + context.block().number()).await?;
                } else {
                    context.ref_state().to_rc()?.nft_create(
                        &object_id,
                        &tx.desc,
                        tx.name.as_str(),
                        &tx.state).await?;
                }
            }
            NFTDesc::FileDesc2((_, parent_id)) => {
                if parent_id.is_some() {
                    return Err(meta_err!(ERROR_PARAM_ERROR));
                }
            }
            NFTDesc::ListDesc(desc) => {
                if desc.content().nft_list.len() != tx.sub_names.len() {
                    return Err(meta_err!(ERROR_PARAM_ERROR));
                }
                if let NFTState::Auctioning((price, coin_id, stop_block)) = nft_state {
                    self.nft_auction.to_rc()?.create_and_auction(
                        &object_id,
                        &tx.desc,
                        nft_name,
                        *price,
                        coin_id,
                        *stop_block + context.block().number() as u64).await?;

                    for (index, sub_nft) in desc.content().nft_list.iter().enumerate() {
                        let sub_id = sub_nft.calculate_id();
                        let sub_nft_desc = NFTDesc::FileDesc2((sub_nft.clone(), Some(object_id.clone())));
                        let sub_name = tx.sub_names.get(index).unwrap();
                        context.ref_state().to_rc()?.nft_create(
                            &sub_id,
                            &sub_nft_desc,
                            sub_name,
                            &NFTState::Normal).await?;
                        context.ref_state().to_rc()?.set_beneficiary(&sub_id, &beneficiary).await?;
                    }
                } else if let NFTState::Selling((price, coin_id, _)) = nft_state {
                    context.ref_state().to_rc()?.nft_create(
                        &object_id,
                        &tx.desc,
                        nft_name,
                        nft_state).await?;

                    if *price == 0 && tx.sub_states.len() != desc.content().nft_list.len() {
                        return Err(meta_err!(ERROR_PARAM_ERROR));
                    }

                    if *price == 0 {
                        for (index, sub_nft) in desc.content().nft_list.iter().enumerate() {
                            let sub_id = sub_nft.calculate_id();
                            let sub_nft_desc = NFTDesc::FileDesc2((sub_nft.clone(), Some(object_id.clone())));
                            let sub_name = tx.sub_names.get(index).unwrap();
                            let sub_nft_state = tx.sub_states.get(index).unwrap();
                            if let NFTState::Selling(_) = sub_nft_state {
                                context.ref_state().to_rc()?.nft_create(
                                    &sub_id,
                                    &sub_nft_desc,
                                    sub_name,
                                    sub_nft_state).await?;
                                context.ref_state().to_rc()?.set_beneficiary(&sub_id, &beneficiary).await?;
                            } else {
                                return Err(meta_err!(ERROR_PARAM_ERROR));
                            }
                        }
                    } else {
                        for (index, sub_nft) in desc.content().nft_list.iter().enumerate() {
                            let sub_id = sub_nft.calculate_id();
                            let sub_nft_desc = NFTDesc::FileDesc2((sub_nft.clone(), Some(object_id.clone())));
                            let sub_name = tx.sub_names.get(index).unwrap();
                            context.ref_state().to_rc()?.nft_create(
                                &sub_id,
                                &sub_nft_desc,
                                sub_name,
                                &NFTState::Selling((0, coin_id.clone(), u64::MAX))).await?;
                            context.ref_state().to_rc()?.set_beneficiary(&sub_id, &beneficiary).await?;
                        }
                    }
                } else {
                    context.ref_state().to_rc()?.nft_create(
                        &object_id,
                        &tx.desc,
                        nft_name,
                        nft_state).await?;

                    for (index, sub_nft) in desc.content().nft_list.iter().enumerate() {
                        let sub_id = sub_nft.calculate_id();
                        let sub_nft_desc = NFTDesc::FileDesc2((sub_nft.clone(), Some(object_id.clone())));
                        let sub_name = tx.sub_names.get(index).unwrap();
                        context.ref_state().to_rc()?.nft_create(
                            &sub_id,
                            &sub_nft_desc,
                            sub_name,
                            &NFTState::Normal).await?;
                        context.ref_state().to_rc()?.set_beneficiary(&sub_id, &beneficiary).await?;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn execute_nft_auction(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTAuctionTx
    ) -> BuckyResult<()> {
        self.nft_auction.to_rc()?.auction(
            &context.caller().id().clone(),
            &tx.nft_id,
            tx.price,
            &tx.coin_id,
            tx.duration_block_num + context.block().number() as u64).await?;

        Ok(())
    }

    pub async fn execute_nft_bid(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTBidTx
    ) -> BuckyResult<()> {
        self.nft_auction.to_rc()?.bid(context.caller().id(), &tx.nft_id, tx.price, &tx.coin_id).await?;
        Ok(())
    }

    pub async fn execute_nft_apply_buy(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTApplyBuyTx
    ) -> BuckyResult<()> {
        log::info!("{} apply buy {} price {}", context.caller().id().to_string(), tx.nft_id.to_string(), tx.price);
        match context.ref_state().to_rc()?.nft_get(&tx.nft_id).await {
            Ok((nft_desc, _, state)) => {
                if let NFTState::Auctioning(_) = state {
                    return Err(meta_err!(ERROR_NFT_IS_AUCTIONING));
                } else if let NFTState::Selling(_) = state {
                    return Err(meta_err!(ERROR_NFT_IS_SELLING));
                }

                if let NFTDesc::FileDesc2((_, parent_id)) = nft_desc {
                    if parent_id.is_some() {
                        let (_, _, parent_state) = context.ref_state().to_rc()?.nft_get(parent_id.as_ref().unwrap()).await?;
                        if let NFTState::Auctioning(_) = parent_state {
                            return Err(meta_err!(ERROR_NFT_IS_AUCTIONING));
                        } else if let NFTState::Selling(_) = parent_state {
                            return Err(meta_err!(ERROR_NFT_IS_SELLING));
                        }
                    }
                } else if let NFTDesc::ListDesc(list_desc) = nft_desc {
                    let beneficiary = context.ref_state().to_rc()?.get_beneficiary(&tx.nft_id).await?;
                    for sub_desc in list_desc.content().nft_list.iter() {
                        let sub_id = sub_desc.calculate_id();

                        match context.ref_state().to_rc()?.nft_get(&sub_id).await {
                            Ok((_, _, state)) => {
                                if let NFTState::Auctioning(_) = state {
                                    return Err(meta_err!(ERROR_NFT_IS_AUCTIONING));
                                } else if let NFTState::Selling(_) = state {
                                    return Err(meta_err!(ERROR_NFT_IS_SELLING));
                                }

                                let sub_beneficiary = context.ref_state().to_rc()?.get_beneficiary(&sub_id).await?;
                                if beneficiary != sub_beneficiary {
                                    return Err(meta_err!(ERROR_NFT_LIST_HAS_SELLED_ANY));
                                }
                            },
                            Err(e) => {
                                if get_meta_err_code(&e)? != ERROR_NOT_FOUND {
                                    return Err(e);
                                }
                            }
                        }
                    }
                }
            },
            Err(e) => {
                if get_meta_err_code(&e)? != ERROR_NOT_FOUND {
                    return Err(e);
                }
            }
        }

        let apply_buy = context.ref_state().to_rc()?.nft_get_apply_buy(&tx.nft_id, context.caller().id()).await?;
        let pay = if apply_buy.is_some() {
            let (latest_price, latest_coin_id) = apply_buy.unwrap();
            if &latest_coin_id == &tx.coin_id {
                if latest_price >= tx.price {
                    return Ok(());
                } else {
                    tx.price - latest_price
                }
            } else {
                return Err(meta_err!(ERROR_NFT_HAS_APPLY_OTHER_COIN));
            }
        } else {
            tx.price
        };
        context.ref_state().to_rc()?.dec_balance(&tx.coin_id, context.caller().id(), pay as i64).await?;
        context.ref_state().to_rc()?.nft_add_apply_buy(&tx.nft_id, context.caller().id(), tx.price, &tx.coin_id).await?;

        let event_params = Event::NFTCancelApplyBuy(NFTCancelApplyBuyParam {
            nft_id: tx.nft_id.clone(),
            user_id: context.caller().id().clone()
        });
        let event_id = hash_data(event_params.to_vec()?.as_slice());
        self.event_manager.to_rc()?.add_or_update_once_event(
            event_id.to_string().as_str(),
            &event_params,
            context.block().number() + self.config.to_rc()?.nft_apply_buy_time()? as i64).await?;

        Ok(())
    }

    pub async fn execute_nft_cancel_apply_buy(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTCancelApplyBuyTx
    ) -> BuckyResult<()> {
        let apply_buy = context.ref_state().to_rc()?.nft_get_apply_buy(&tx.nft_id, context.caller().id()).await?;
        if apply_buy.is_some() {
            let (price, coin_id) = apply_buy.unwrap();
            context.ref_state().to_rc()?.inc_balance(&coin_id, context.caller().id(), price as i64).await?;
            context.ref_state().to_rc()?.nft_remove_apply_buy(&tx.nft_id, context.caller().id()).await?;

            let event_params = Event::NFTCancelApplyBuy(NFTCancelApplyBuyParam {
                nft_id: tx.nft_id.clone(),
                user_id: context.caller().id().clone()
            });
            let event_id = hash_data(event_params.to_vec()?.as_slice());
            self.event_manager.to_rc()?.drop_once_event(event_id.to_string().as_str()).await?;
        }

        Ok(())
    }

    pub async fn execute_nft_agree_apply_buy(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTAgreeApplyTx
    ) -> BuckyResult<()> {
        log::info!("{} agree user {} apply buy {}", context.caller().id().to_string(), tx.user_id.to_string(), tx.nft_id.to_string());
        let (nft_desc, _, state) = context.ref_state().to_rc()?.nft_get(&tx.nft_id).await?;
        let beneficiary = context.ref_state().to_rc()?.get_beneficiary(&tx.nft_id).await?;
        if let NFTState::Auctioning(_) = state {
            return Err(meta_err!(ERROR_NFT_IS_AUCTIONING));
        } else if let NFTState::Selling(_) = state {
            return Err(meta_err!(ERROR_NFT_IS_SELLING));
        }

        if &beneficiary != context.caller().id() {
            return Err(meta_err!(ERROR_ACCESS_DENIED));
        }

        let balance = context.ref_state().to_rc()?.get_balance(&tx.nft_id, &CoinTokenId::Coin(0)).await?;
        if balance > 0 {
            context.ref_state().to_rc()?.dec_balance(&CoinTokenId::Coin(0), &tx.nft_id, balance).await?;
            context.ref_state().to_rc()?.inc_balance(&CoinTokenId::Coin(0), &beneficiary, balance).await?;
        }

        let apply_buy = context.ref_state().to_rc()?.nft_get_apply_buy(&tx.nft_id, &tx.user_id).await?;
        if apply_buy.is_some() {
            let (price, coin_id) = apply_buy.unwrap();
            context.ref_state().to_rc()?.inc_balance(&coin_id, context.caller().id(), price as i64).await?;
            context.ref_state().to_rc()?.set_beneficiary(&tx.nft_id, &tx.user_id).await?;

            let list = context.ref_state().to_rc()?.nft_get_apply_buy_list(&tx.nft_id, 0, i64::MAX).await?;
            for (buyer_id, price, coin_id) in list.iter() {
                let event_params = Event::NFTCancelApplyBuy(NFTCancelApplyBuyParam {
                    nft_id: tx.nft_id.clone(),
                    user_id: buyer_id.clone()
                });
                let event_id = hash_data(event_params.to_vec()?.as_slice());
                self.event_manager.to_rc()?.drop_once_event(event_id.to_string().as_str()).await?;

                if buyer_id == &tx.user_id {
                    continue;
                }
                context.ref_state().to_rc()?.inc_balance(coin_id, buyer_id, *price as i64).await?;
            }
            context.ref_state().to_rc()?.nft_remove_all_apply_buy(&tx.nft_id).await?;

            if let NFTDesc::FileDesc2((_, parent_id)) = nft_desc {
                if parent_id.is_some() {
                    let list = context.ref_state().to_rc()?.nft_get_apply_buy_list(parent_id.as_ref().unwrap(), 0, i64::MAX).await?;
                    for (buyer_id, price, coin_id) in list.iter() {
                        let event_params = Event::NFTCancelApplyBuy(NFTCancelApplyBuyParam {
                            nft_id: parent_id.clone().unwrap(),
                            user_id: buyer_id.clone()
                        });
                        let event_id = hash_data(event_params.to_vec()?.as_slice());
                        self.event_manager.to_rc()?.drop_once_event(event_id.to_string().as_str()).await?;
                        context.ref_state().to_rc()?.inc_balance(coin_id, buyer_id, *price as i64).await?;
                    }
                    context.ref_state().to_rc()?.nft_remove_all_apply_buy(parent_id.as_ref().unwrap()).await?;
                }
            } else if let NFTDesc::ListDesc(sub_list) = nft_desc {
                for sub_desc in sub_list.content().nft_list.iter() {
                    let sub_id = sub_desc.calculate_id();

                    let balance = context.ref_state().to_rc()?.get_balance(&sub_id, &CoinTokenId::Coin(0)).await?;
                    if balance > 0 {
                        context.ref_state().to_rc()?.dec_balance(&CoinTokenId::Coin(0), &sub_id, balance).await?;
                        context.ref_state().to_rc()?.inc_balance(&CoinTokenId::Coin(0), &beneficiary, balance).await?;
                    }

                    let list = context.ref_state().to_rc()?.nft_get_apply_buy_list(&sub_id, 0, i64::MAX).await?;
                    for (buyer_id, price, coin_id) in list.iter() {
                        let event_params = Event::NFTCancelApplyBuy(NFTCancelApplyBuyParam {
                            nft_id: sub_id.clone(),
                            user_id: buyer_id.clone()
                        });
                        let event_id = hash_data(event_params.to_vec()?.as_slice());
                        self.event_manager.to_rc()?.drop_once_event(event_id.to_string().as_str()).await?;
                        context.ref_state().to_rc()?.inc_balance(coin_id, buyer_id, *price as i64).await?;
                    }
                    context.ref_state().to_rc()?.nft_remove_all_apply_buy(&sub_id).await?;
                }
            }
            Ok(())
        } else {
            return Err(meta_err!(ERROR_NFT_USER_NOT_APPLY_BUY));
        }
    }

    pub async fn execute_nft_sell(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTSellTx
    ) -> BuckyResult<()> {
        log::info!("{} sell nft {} price {} duration {}", context.caller().id().to_string(), tx.nft_id.to_string(), tx.price, tx.duration_block_num);
        let (nft_desc, _, state) = context.ref_state().to_rc()?.nft_get(&tx.nft_id).await?;
        let beneficiary = context.ref_state().to_rc()?.get_beneficiary(&tx.nft_id).await?;
        if &beneficiary != context.caller().id() {
            return Err(meta_err!(ERROR_ACCESS_DENIED));
        }

        if let NFTState::Auctioning(_) = state {
            return Err(meta_err!(ERROR_NFT_IS_AUCTIONING));
        }

        if let NFTDesc::FileDesc2((_, parent_id)) = nft_desc {
            if parent_id.is_some() {
                let parent_beneficiary = context.ref_state().to_rc()?.get_beneficiary(parent_id.as_ref().unwrap()).await?;
                if beneficiary == parent_beneficiary {
                    return Err(meta_err!(ERROR_NOT_SUPPORT));
                }
            }
        } else if let NFTDesc::ListDesc(list_desc) = nft_desc {
            for sub_desc in list_desc.content().nft_list.iter() {
                let sub_id = sub_desc.calculate_id();
                let sub_beneficiary = context.ref_state().to_rc()?.get_beneficiary(&sub_id).await?;
                if sub_beneficiary != beneficiary {
                    return Err(meta_err!(ERROR_NOT_SUPPORT));
                }

                let state = NFTState::Selling((0, tx.coin_id.clone(), u64::MAX));
                context.ref_state().to_rc()?.nft_update_state(&sub_id, &state).await?;

                let apply_list = context.ref_state().to_rc()?.nft_get_apply_buy_list(&sub_id, 0, i64::MAX).await?;
                for (buyer_id, price, coin_id) in apply_list.iter() {
                    let event_params = Event::NFTCancelApplyBuy(NFTCancelApplyBuyParam {
                        nft_id: sub_id.clone(),
                        user_id: buyer_id.clone(),
                    });
                    let event_id = hash_data(event_params.to_vec()?.as_slice());
                    self.event_manager.to_rc()?.drop_once_event(event_id.to_string().as_str()).await?;
                    context.ref_state().to_rc()?.inc_balance(coin_id, buyer_id, *price as i64).await?;
                }
                context.ref_state().to_rc()?.nft_remove_all_apply_buy(&sub_id).await?;
            }
        }


        let state = NFTState::Selling((tx.price, tx.coin_id.clone(), context.block().number() as u64 + tx.duration_block_num));
        context.ref_state().to_rc()?.nft_update_state(&tx.nft_id, &state).await?;

        let event_params = Event::NFTStopSell(NFTStopSell {
            nft_id: tx.nft_id.clone()
        });
        let event_id = hash_data(event_params.to_vec()?.as_slice());
        self.event_manager.to_rc()?.add_or_update_once_event(
            event_id.to_string().as_str(),
            &event_params,
            context.block().number() + tx.duration_block_num as i64).await?;

        let apply_list = context.ref_state().to_rc()?.nft_get_apply_buy_list(&tx.nft_id, 0, i64::MAX).await?;
        for (buyer_id, price, coin_id) in apply_list.iter() {
            let event_params = Event::NFTCancelApplyBuy(NFTCancelApplyBuyParam {
                nft_id: tx.nft_id.clone(),
                user_id: buyer_id.clone(),
            });
            let event_id = hash_data(event_params.to_vec()?.as_slice());
            self.event_manager.to_rc()?.drop_once_event(event_id.to_string().as_str()).await?;
            context.ref_state().to_rc()?.inc_balance(coin_id, buyer_id, *price as i64).await?;
        }
        context.ref_state().to_rc()?.nft_remove_all_apply_buy(&tx.nft_id).await?;
        Ok(())
    }

    pub async fn execute_nft_sell2(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTSellTx2
    ) -> BuckyResult<()> {
        let (nft_desc, _, state) = context.ref_state().to_rc()?.nft_get(&tx.nft_id).await?;
        let beneficiary = context.ref_state().to_rc()?.get_beneficiary(&tx.nft_id).await?;
        if &beneficiary != context.caller().id() {
            return Err(meta_err!(ERROR_ACCESS_DENIED));
        }

        if let NFTState::Auctioning(_) = state {
            return Err(meta_err!(ERROR_NFT_IS_AUCTIONING));
        }

        if let NFTDesc::FileDesc2((_, parent_id)) = nft_desc {
            if parent_id.is_some() {
                let parent_beneficiary = context.ref_state().to_rc()?.get_beneficiary(parent_id.as_ref().unwrap()).await?;
                if beneficiary == parent_beneficiary {
                    return Err(meta_err!(ERROR_NOT_SUPPORT));
                }
            }
        } else if let NFTDesc::ListDesc(list_desc) = nft_desc {
            if tx.price == 0 && tx.sub_sell_infos.len() != list_desc.content().nft_list.len() {
                return Err(meta_err!(ERROR_PARAM_ERROR));
            }
            for (index, sub_desc) in list_desc.content().nft_list.iter().enumerate() {
                let sub_id = sub_desc.calculate_id();
                let sub_beneficiary = context.ref_state().to_rc()?.get_beneficiary(&sub_id).await?;
                if sub_beneficiary != beneficiary {
                    return Err(meta_err!(ERROR_NOT_SUPPORT));
                }

                let state = if tx.price == 0 {
                    let (sub_coin_id, sub_price) = tx.sub_sell_infos.get(index).unwrap();
                    NFTState::Selling((*sub_price, sub_coin_id.clone(), u64::MAX))
                } else {
                    NFTState::Selling((0, tx.coin_id.clone(), u64::MAX))
                };
                context.ref_state().to_rc()?.nft_update_state(&sub_id, &state).await?;
            }
        }


        let state = NFTState::Selling((tx.price, tx.coin_id.clone(), u64::MAX));
        context.ref_state().to_rc()?.nft_update_state(&tx.nft_id, &state).await?;
        Ok(())
    }

    pub async fn execute_nft_cancel_sell(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTCancelSellTx,
    ) -> BuckyResult<()> {
        let (nft_desc, _, state) = context.ref_state().to_rc()?.nft_get(&tx.nft_id).await?;
        let beneficiary = context.ref_state().to_rc()?.get_beneficiary(&tx.nft_id).await?;
        if &beneficiary != context.caller().id() {
            return Err(meta_err!(ERROR_ACCESS_DENIED));
        }

        if let NFTState::Auctioning(_) = state {
            return Err(meta_err!(ERROR_NFT_IS_AUCTIONING));
        } else if NFTState::Normal == state {
            return Err(meta_err!(ERROR_NFT_IS_NORMAL));
        }

        if let NFTDesc::FileDesc2((_, parent_id)) = nft_desc {
            if parent_id.is_some() {
                let parent_beneficiary = context.ref_state().to_rc()?.get_beneficiary(parent_id.as_ref().unwrap()).await?;
                if beneficiary == parent_beneficiary {
                    return Err(meta_err!(ERROR_NOT_SUPPORT));
                }
            }
        } else if let NFTDesc::ListDesc(list_desc) = nft_desc {
            for sub_desc in list_desc.content().nft_list.iter() {
                let sub_id = sub_desc.calculate_id();
                let sub_beneficiary = context.ref_state().to_rc()?.get_beneficiary(&sub_id).await?;
                if sub_beneficiary != beneficiary {
                    continue;
                }
                context.ref_state().to_rc()?.nft_update_state(&sub_id, &NFTState::Normal).await?;
            }
        }

        let event_params = Event::NFTStopSell(NFTStopSell {
            nft_id: tx.nft_id.clone()
        });
        let id = hash_data(event_params.to_vec()?.as_slice());
        self.event_manager.to_rc()?.drop_once_event(id.to_string().as_str()).await?;

        context.ref_state().to_rc()?.nft_update_state(&tx.nft_id, &NFTState::Normal).await?;
        Ok(())
    }

    pub async fn execute_nft_buy(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTBuyTx
    ) -> BuckyResult<()> {
        log::info!("{} buy nft {} price {}", context.caller().id().to_string(), tx.nft_id.to_string(), tx.price);
        let (nft_desc, _, state) = context.ref_state().to_rc()?.nft_get(&tx.nft_id).await?;
        if let NFTState::Selling((mut price, coin_id, _)) = state {
            let beneficiary = context.ref_state().to_rc()?.get_beneficiary(&tx.nft_id).await?;
            if context.caller().id() == &beneficiary {
                return Err(meta_err!(ERROR_NFT_IS_OWNER));
            }

            if let NFTDesc::FileDesc2((_, parent_id)) = nft_desc {
                if parent_id.is_some() {
                    let parent_beneficiary = context.ref_state().to_rc()?.get_beneficiary(parent_id.as_ref().unwrap()).await?;
                    if parent_beneficiary == beneficiary {
                        if let NFTState::Selling((price, _, _)) = state {
                            if price == 0 {
                                return Err(meta_err!(ERROR_NFT_IS_SUB));
                            }
                        }
                    }
                }
            } else if let NFTDesc::ListDesc(sub_list) = nft_desc {
                let mut sum_price = 0;
                for sub_nft in sub_list.content().nft_list.iter() {
                    let sub_id = sub_nft.calculate_id();
                    let sub_benefi = context.ref_state().to_rc()?.get_beneficiary(&sub_id).await?;
                    if sub_benefi != beneficiary {
                        continue;
                    }
                    let (_, _, sub_state) = context.ref_state().to_rc()?.nft_get(&sub_id).await?;
                    if let NFTState::Selling((sub_price, _, _)) = sub_state {
                        sum_price += sub_price;
                    } else {
                        continue;
                    }
                    let balance = context.ref_state().to_rc()?.get_balance(&sub_id, &CoinTokenId::Coin(0)).await?;
                    if balance > 0 {
                        context.ref_state().to_rc()?.dec_balance(&CoinTokenId::Coin(0), &sub_id, balance).await?;
                        context.ref_state().to_rc()?.inc_balance(&CoinTokenId::Coin(0), &beneficiary, balance).await?;
                    }
                    context.ref_state().to_rc()?.set_beneficiary(&sub_id, context.caller().id()).await?;
                    context.ref_state().to_rc()?.nft_update_state(&sub_id, &NFTState::Normal).await?;
                }
                if price == 0 {
                    price = sum_price;
                }
            }

            if tx.coin_id != coin_id {
                Err(meta_err!(ERROR_BID_COIN_NOT_MATCH))
            } else if tx.price < price {
                Err(meta_err!(ERROR_BID_PRICE_TOO_LOW))
            } else {
                let balance = context.ref_state().to_rc()?.get_balance(&tx.nft_id, &CoinTokenId::Coin(0)).await?;
                if balance > 0 {
                    context.ref_state().to_rc()?.dec_balance(&CoinTokenId::Coin(0), &tx.nft_id, balance).await?;
                    context.ref_state().to_rc()?.inc_balance(&CoinTokenId::Coin(0), &beneficiary, balance).await?;
                }
                context.ref_state().to_rc()?.dec_balance(&coin_id, context.caller().id(), price as i64).await?;
                context.ref_state().to_rc()?.inc_balance(&coin_id, &beneficiary, price as i64).await?;
                context.ref_state().to_rc()?.set_beneficiary(&tx.nft_id, context.caller().id()).await?;
                context.ref_state().to_rc()?.nft_update_state(&tx.nft_id, &NFTState::Normal).await?;

                let event_params = Event::NFTStopSell(NFTStopSell {
                    nft_id: tx.nft_id.clone()
                });
                let id = hash_data(event_params.to_vec()?.as_slice());
                self.event_manager.to_rc()?.drop_once_event(id.to_string().as_str()).await?;

                Ok(())
            }
        } else {
            Err(meta_err!(ERROR_NFT_IS_NOT_SELLING))
        }
    }

    pub async fn execute_nft_like(
        &self,
        _context: &mut ExecuteContext,
        _tx: &NFTLikeTx
    ) -> BuckyResult<()> {
        Ok(())
    }

    pub async fn execute_nft_set_name(
        &self,
        context: &mut ExecuteContext,
        tx: &NFTSetNameTx
    ) -> BuckyResult<()> {
        let _ = context.ref_state().to_rc()?.nft_get(&tx.nft_id).await?;
        let beneficiary = context.ref_state().to_rc()?.get_beneficiary(&tx.nft_id).await?;
        if &beneficiary != context.caller().id() {
            return Err(meta_err!(ERROR_ACCESS_DENIED));
        }

        context.ref_state().to_rc()?.nft_set_name(&tx.nft_id, tx.name.as_str()).await?;

        Ok(())
    }
}

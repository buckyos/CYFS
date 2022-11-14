use crate::executor::tx_executor::TxExecutor;
use crate::executor::transaction::ExecuteContext;
use crate::executor::context;
use cyfs_base::*;
use crate::{AccountInfo, ArcWeakHelper};
use crate::*;
use crate::mint::subchain_mint::SubChainMint;
use std::str::FromStr;
use crate::sub_chain_helper::MetaClient;
use std::time::Duration;

impl TxExecutor {
    pub async fn execute_subchain_create_account(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, miner_group: &MinerGroup) -> BuckyResult<()> {
        let desc_signs = miner_group.signs().desc_signs();
        if desc_signs.is_none() || desc_signs.unwrap().len() == 0 {
            log::error!("org {} desc don't sign", miner_group.desc().calculate_id());
            return Err(meta_err!(ERROR_SIGNATURE_ERROR))
        }

        let body_signs = miner_group.signs().body_signs();
        if body_signs.is_none() || body_signs.unwrap().len() == 0 {
            log::error!("org {} obj don't sign", miner_group.desc().calculate_id());
            return Err(meta_err!(ERROR_SIGNATURE_ERROR))
        }

        let members = miner_group.members();
        for member in members {
            let device_id = member.calculate_id();
            let mut verify = false;
            for desc_sign in desc_signs.unwrap() {
                match desc_sign.sign_source() {
                    SignatureSource::Object(linker) => {
                        if linker.obj_id == device_id {
                            let verifier = RsaCPUObjectVerifier::new(member.public_key().clone());
                            verify = verify_object_desc_sign(&verifier, miner_group, desc_sign).await?;
                            break;
                        }
                    }
                    _ => {
                        return Err(meta_err!(ERROR_SIGNATURE_ERROR));
                    }
                }
            }
            if !verify {
                log::error!("{} signature verify failed", device_id.to_string());
                return Err(meta_err!(ERROR_SIGNATURE_ERROR));
            }

            let mut verify = false;
            for body_sign in body_signs.unwrap() {
                match body_sign.sign_source() {
                    SignatureSource::Object(linker) => {
                        if linker.obj_id == device_id {
                            let verifier = RsaCPUObjectVerifier::new(member.public_key().clone());
                            verify = verify_object_body_sign(&verifier, miner_group, body_sign).await?;
                            break;
                        }
                    }
                    _ => {
                        return Err(meta_err!(ERROR_SIGNATURE_ERROR));
                    }
                }
            }
            if !verify {
                log::error!("{} signature verify failed", device_id.to_string());
                return Err(meta_err!(ERROR_SIGNATURE_ERROR));
            }

            context.ref_state().to_rc()?.add_account_info(&AccountInfo::Device(member.clone())).await?;
        }
        context.ref_state().to_rc()?.add_account_info(&AccountInfo::MinerGroup(miner_group.clone())).await?;
        Ok(())
    }

    pub async fn execute_subchain_update_account(&self, _context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, _miner_group: &MinerGroup) -> BuckyResult<()> {
        Ok(())
    }

    pub async fn execute_subchain_withdraw(&self, _tx: &MetaTx, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, withdraw_tx: &SubChainWithdrawTx) -> BuckyResult<()> {
        let subchain = context.ref_state().to_rc()?.get_account_info(&withdraw_tx.subchain_id).await?;
        if let AccountInfo::MinerGroup(miner_group) = subchain {
            if !miner_group.has_member(context.caller().id()) {
                log::error!("group {} has not {}", withdraw_tx.subchain_id.to_string(), context.caller().id().to_string());
                return Err(meta_err!(ERROR_SIGNATURE_ERROR));
            }

            let ret = MetaTx::clone_from_slice(withdraw_tx.withdraw_tx.as_slice());
            if ret.is_err() {
                log::error!("subchain withdraw tx type err!");
                return Err(meta_err!(ERROR_SUBCHAIN_WITHDRAW_TX_ERROR));
            }
            let subchain_tx = ret.unwrap();
            if subchain_tx.desc().content().body.get_obj().len() != 1 {
                log::error!("subchain withdraw tx err!");
                return Err(meta_err!(ERROR_SUBCHAIN_WITHDRAW_TX_ERROR));
            }

            if let MetaTxBody::WithdrawFromSubChain(subchain_withdraw_tx) = &subchain_tx.desc().content().body.get_obj()[0] {
                let subchain_tx_id = subchain_tx.desc().calculate_id();
                let caller_id = subchain_tx.desc().content().caller.id()?;
                let ret = context.ref_state().to_rc()?.get_subchain_withdraw_record(&withdraw_tx.subchain_id, &subchain_tx_id).await;
                if let Err(e) = &ret {
                     if get_meta_err_code(e)? == ERROR_NOT_FOUND {
                         let vote_list = vec![context.caller().id().clone()];
                         context.ref_state().to_rc()?.create_subchain_withdraw_record(&withdraw_tx.subchain_id, &subchain_tx_id, vote_list.to_vec()?).await?;
                         return Ok(());
                     } else {
                         return Err(ret.err().unwrap());
                     }
                } else {
                    let mut vote_list = Vec::<ObjectId>::clone_from_slice(ret.unwrap().as_slice())?;
                    let mut find = false;
                    for vote_id in &vote_list {
                        if vote_id == context.caller().id() {
                            find = true;
                            break;
                        }
                    }

                    if !find {
                        vote_list.push(context.caller().id().clone());
                        context.ref_state().to_rc()?.update_subchain_withdraw_record(&withdraw_tx.subchain_id, &subchain_tx_id, vote_list.to_vec()?).await?;

                        if vote_list.len() == (0.7 * miner_group.members().len() as f32).ceil() as usize {
                            context.ref_state().to_rc()?.inc_balance(&subchain_withdraw_tx.coin_id, &caller_id, subchain_withdraw_tx.value).await?;
                            context.ref_state().to_rc()?.dec_balance(&subchain_withdraw_tx.coin_id,&withdraw_tx.subchain_id, subchain_withdraw_tx.value).await?;
                        }
                    }
                    return Ok(());
                }
            } else {
                log::error!("subchain withdraw tx err!");
                return Err(meta_err!(ERROR_SUBCHAIN_WITHDRAW_TX_ERROR));
            }

        } else {
            log::error!("account {} is not chain", withdraw_tx.subchain_id.to_string());
            return Err(meta_err!(ERROR_SIGNATURE_ERROR));
        }
    }

    pub async fn execute_subchain_coinage(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, coinage_tx: &SubChainCoinageRecordTx) -> BuckyResult<()> {
        let org_id = context.ref_state().to_rc()?.config_get("miners_group", "").await?;
        let subchain_mint = SubChainMint::new(ObjectId::from_str(org_id.as_str())?,
        &context.ref_state().to_rc()?,
        &context.config().to_rc()?, self.mint_url.clone());
        if subchain_mint.check_coinage_record(coinage_tx).await? {
            subchain_mint.execute_coinage_record(coinage_tx).await
        } else {
            Err(meta_err!(ERROR_INVALID))
        }
    }

    pub async fn execute_withdraw_from_sub_chain(&self, tx: &MetaTx, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, withdraw_tx: &WithdrawFromSubChainTx) -> BuckyResult<()> {
        context.ref_state().to_rc()?.dec_balance(&withdraw_tx.coin_id, context.caller().id(), withdraw_tx.value).await?;

        if self.miner_key.is_some() {
            let org_id = context.ref_state().to_rc()?.config_get("miners_group", "").await?;
            log::info!("execute_withdraw_from_sub_chain org_id:{} miner:{}", org_id.as_str(), context.block().coinbase().to_string());
            let meta_client = MetaClient::new(self.mint_url.as_str());
            loop {
                let ret = meta_client.sub_chain_withdraw(TxCaller::Id(self.miner_id.clone()),
                                                       ObjectId::from_str(org_id.as_str())?,
                                                         tx,
                                                       self.miner_key.as_ref().unwrap()).await;
                if ret.is_ok() {
                    break;
                }

                async_std::task::sleep(Duration::new(1, 0)).await;
            }
            Ok(())
        } else {
            Err(meta_err!(ERROR_INVALID))
        }
    }
}

#[cfg(test)]
mod test_sub_chain_tx {
    use std::path::Path;
    use crate::{new_archive_storage, NFTAuction, sql_storage_tests, State};
    use crate::executor::context::{Config, UnionWithdrawManager};
    use crate::events::event_manager::EventManager;
    use crate::rent::rent_manager::RentManager;
    use crate::name_auction::auction::Auction;
    use crate::executor::tx_executor::TxExecutor;
    use cyfs_base::*;
    use cyfs_base_meta::*;
    use crate::chain::bft_miner_test::{create_miner_device_info_list, create_bft_org, init_test_log};

    #[test]
    fn test_create_sub_chain() {
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
                                           &union_withdraw_manager, &nft_auction, new_archive_storage(Path::new(""), false).create_archive(false).await, "http://127.0.0.1:11998".to_owned(), None, ObjectId::default(), true);


            let private_key = PrivateKey::generate_rsa(1024).unwrap();
            let device = Device::new(
                None
                , UniqueId::default()
                , Vec::new()
                , Vec::new()
                , Vec::new()
                , private_key.public()
                , Area::default()
                , DeviceCategory::OOD).build();
            let id = device.desc().calculate_id();


            let device_list = create_miner_device_info_list(7);
            let (device1, private_key1) = &device_list[0];
            let org = create_bft_org(&device_list).unwrap();

            let ret = state.inc_balance(&CoinTokenId::Coin(0), &org.desc().calculate_id(), 100000000000).await;
            assert!(ret.is_ok());

            let mut nonce1 = 1;
            let mut meta_tx = MetaTx::new(nonce1, TxCaller::Device(device1.desc().clone()), 0, 0, 0, None,
            MetaTxBody::CreateSubChainAccount(org.clone()), Vec::new()).build();
            nonce1 += 1;
            meta_tx.async_sign(private_key1.clone()).await.unwrap();

            let block_desc = BlockDesc::new(BlockDescContent::new(id.clone(), None)).build();
            let ret = executor.execute(&block_desc, &meta_tx, None).await;
            assert!(ret.is_ok());

            let mut withdraw_from_subchain_tx = MetaTx::new(1,
                                                        TxCaller::Device(device.desc().clone()),
            0,
            0,
            0,
            None,
            MetaTxBody::WithdrawFromSubChain(WithdrawFromSubChainTx {
                coin_id: CoinTokenId::Coin(0),
                value: 100000000
            }),
            Vec::new()).build();

            withdraw_from_subchain_tx.async_sign(private_key).await.unwrap();

            let mut index = 0;
            for (device, private) in &device_list {
                let mut tx = if index == 0 {
                    MetaTx::new(nonce1,
                    TxCaller::Device(device.desc().clone()),
                    0,
                    0,
                    0,
                    None,
                    MetaTxBody::SubChainWithdraw(SubChainWithdrawTx{
                        subchain_id: org.desc().calculate_id(),
                        withdraw_tx: withdraw_from_subchain_tx.to_vec().unwrap() }),
                    Vec::new()).build()
                } else {
                    MetaTx::new(1,
                                TxCaller::Device(device.desc().clone()),
                                0,
                                0,
                                0,
                                None,
                                MetaTxBody::SubChainWithdraw(SubChainWithdrawTx{
                                    subchain_id: org.desc().calculate_id(),
                                    withdraw_tx: withdraw_from_subchain_tx.to_vec().unwrap() }),
                                Vec::new()).build()
                };
                tx.async_sign(private.clone()).await.unwrap();
                index += 1;
                let ret = executor.execute(&block_desc, &tx, None).await;
                assert!(ret.is_ok());
            }

            let org_value = state.get_balance(&org.desc().calculate_id(), &CoinTokenId::Coin(0)).await.unwrap();
            assert_eq!(org_value, 99900000000);

            let device_value = state.get_balance(&id, &CoinTokenId::Coin(0)).await.unwrap();
            assert_eq!(device_value, 100000000);
        })
    }
}

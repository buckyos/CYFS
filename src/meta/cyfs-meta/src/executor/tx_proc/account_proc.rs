use crate::executor::context;
use crate::executor::context::UnionAccountState;
use crate::executor::transaction::ExecuteContext;
use crate::executor::tx_executor::TxExecutor;
use crate::helper::{get_meta_err_code, ArcWeakHelper};
use crate::meta_backend::MetaBackend;
use crate::State;
use cyfs_base::*;
use cyfs_base_meta::*;
use std::convert::TryFrom;

impl TxExecutor {
    pub async fn execute_trans_balance(
        &self,
        context: &mut ExecuteContext,
        fee_counter: &mut context::FeeCounter,
        tx: &TransBalanceTx,
        _backend: &mut MetaBackend,
        _config: &evm::Config,
    ) -> BuckyResult<Vec<TxLog>> {
        if let context::AccountMethods::Single(methods) = context.caller().methods() {
            let logs = methods
                .trans_balance(fee_counter, &tx.ctid, &tx.to /*, backend, config*/)
                .await?;

            // 下边是扣除租金的相关逻辑
            for (to_id, _v) in &tx.to {
                //TODO:暂时移除其他人充值防删除逻辑
                // let desc_obj_ret = self.ref_state.to_rc()?.get_obj_desc(to_id).await;
                // if let Ok(desc_obj) = desc_obj_ret {
                //     if let SavedMetaObject::File(file) = desc_obj {
                //         if file.desc().owner().as_ref().unwrap() != context.caller().id() {
                //             let mut desc_state = self.ref_state.to_rc()?.get_desc_extra(to_id).await?;
                //             if let CoinTokenId::Coin(coin_id) = &tx.ctid {
                //                 if desc_state.coin_id == *coin_id {
                //                     desc_state.other_charge_balance += v;
                //                     self.ref_state.to_rc()?.update_desc_extra(&desc_state)?;
                //                 }
                //             }
                //         }
                //     }
                // }

                self.rent_manager
                    .to_rc()?
                    .check_and_deduct_rent_arrears_for_desc(context.block(), &to_id, &tx.ctid)
                    .await?;

                let owned_names = self.ref_state.to_rc()?.get_owned_names(to_id).await?;
                for name in owned_names {
                    let mut name_state = self
                        .ref_state
                        .to_rc()?
                        .get_name_state(name.as_str())
                        .await?;
                    if name_state == NameState::Lock {
                        let rent_state = self
                            .rent_manager
                            .to_rc()?
                            .check_and_deduct_rent_arrears_for_name(context.block(), name.as_str())
                            .await?;
                        if rent_state.rent_arrears == 0 {
                            name_state = NameState::Normal;
                            self.ref_state
                                .to_rc()?
                                .update_name_state(name.as_str(), name_state)
                                .await?;
                        }
                    }
                }
                //
                // let unpaid_list = self.ref_state.to_rc()?.get_unpaid_records(&to_id, &tx.ctid).await?;
                // for record in unpaid_list {
                //     let ret = self.ref_state.to_rc()?.dec_balance(&record.coin_id, &record.account_id, record.amount as i64).await;
                //     if ret.is_ok() {
                //         self.ref_state.to_rc()?.inc_balance(&record.coin_id, &record.to, record.amount as i64).await?;
                //         self.ref_state.to_rc()?.drop_unpaid_record(record.id, &record.coin_id).await?;
                //     } else {
                //         break;
                //     }
                // }
            }
            Ok(logs)
        } else {
            Err(crate::meta_err!(ERROR_INVALID))
        }
    }

    pub async fn execute_withdraw_to_owner(
        &self,
        context: &mut ExecuteContext,
        _fee_counter: &mut context::FeeCounter,
        tx: &WithdrawToOwner,
    ) -> BuckyResult<()> {
        let beneficiary = self.ref_state.to_rc()?.get_beneficiary(&tx.id).await?;
        let owner = if beneficiary != tx.id {
            Some(beneficiary)
        } else {
            let desc_obj_ret = self.ref_state.to_rc()?.get_obj_desc(&tx.id).await?;
            match desc_obj_ret {
                SavedMetaObject::Device(device) => device.desc().owner().clone(),
                SavedMetaObject::People(_) => None,
                SavedMetaObject::UnionAccount(_) => None,
                SavedMetaObject::Group(_) => None,
                SavedMetaObject::File(file) => file.desc().owner().clone(),
                SavedMetaObject::Data(data) => {
                    let ret = AnyNamedObject::clone_from_slice(data.data.as_slice());
                    if ret.is_ok() {
                        ret.unwrap().owner().clone()
                    } else {
                        None
                    }
                }
                SavedMetaObject::MinerGroup(_) => None,
                SavedMetaObject::SNService(_) => None,
                SavedMetaObject::Contract(contract) => contract.desc().owner().clone(),
                SavedMetaObject::SimpleGroup => {
                    panic!("SimpleGroup is deprecated, you can use the Group.")
                }
                SavedMetaObject::Org => panic!("Org is deprecated, you can use the Group."),
            }
        };
        if owner.is_some() && owner.as_ref().unwrap() == context.caller().id() {
            self.ref_state
                .to_rc()?
                .dec_balance(&tx.ctid, &tx.id, tx.value)
                .await?;
            self.ref_state
                .to_rc()?
                .inc_balance(&tx.ctid, &context.caller().id(), tx.value)
                .await?;
            return Ok(());
        }
        Err(crate::meta_err!(ERROR_INVALID))
    }

    //理解下面逻辑需了解ffs-meta的闪电网络工作原理
    // 1.双方决定建立union account
    // 2.由任一方执行CreateUnionTX，在Meta上建立union account(需要持续付租金)
    // 3.另一方可按需要执行TransBalanceTX，在union account中增加余额
    // 4.双方进行链下交易，不断地创建有双签名的SetUnionTx,Set操作的left + right的和总是为0
    // 5.任何一方都可以在必要的时候，将有双签名的SetUnionTx上链条，只有最新的SetUnionTx会生效
    //   操作完成后，UnionAccount中的余额状态会公开的改变
    // 6.任何一方，都可以在必要的时候，调用带return flag的SetUnionTx，将UnionAccount中所属自己的余额反还。
    //   改操作在上链后永远不会立刻生效，而是需要等待一定的时间后才会操作UnionAccount
    // 7.当一方发现另一方提交了单签名的SetUnionTx,可以提交最新的,有多签名的SetUnionTx来保障自己的收益
    // 综上：使用闪电网络，需要最少3个操作，创建->使用->返还
    // 问题：union account的租金谁出？ 还是和Address Account一样，是不需要租金，永久保存的特殊存在？
    // 扩展：包含超过2个object的超级union account？

    pub async fn execute_create_union(
        &self,
        context: &mut ExecuteContext,
        fee_counter: &mut context::FeeCounter,
        tx: &CreateUnionTx,
    ) -> BuckyResult<()> {
        let union_account = &tx.body.account;
        let mut public_key_list = Vec::new();
        let ret = context
            .ref_state()
            .to_rc()?
            .get_obj_desc(&union_account.desc().content().left())
            .await;
        if let Err(err) = &ret {
            let err_code = get_meta_err_code(err)?;
            return if err_code == ERROR_NOT_FOUND {
                Err(crate::meta_err!(ERROR_CANT_FIND_LEFT_USER_DESC))
            } else {
                Err(crate::meta_err2!(err_code, err.msg()))
            };
        }
        let left_account = ret.unwrap();
        match left_account {
            SavedMetaObject::Device(device) => public_key_list.push((
                union_account.desc().content().left().clone(),
                device.desc().public_key().clone(),
            )),
            // SavedMetaObject::People(people) => {
            //     public_key_list.push((union_account.desc().content().right().clone(), people.desc()))
            // }
            _ => return Err(crate::meta_err!(ERROR_LEFT_ACCOUNT_TYPE)),
        }

        let ret = context
            .ref_state()
            .to_rc()?
            .get_obj_desc(&union_account.desc().content().right())
            .await;
        if let Err(err) = &ret {
            let err_code = get_meta_err_code(err)?;
            return if err_code == ERROR_NOT_FOUND {
                Err(crate::meta_err!(ERROR_CANT_FIND_RIGHT_USER_DESC))
            } else {
                Err(crate::meta_err2!(err_code, err.msg()))
            };
        }
        let right_account = ret.unwrap();
        match right_account {
            SavedMetaObject::Device(device) => public_key_list.push((
                union_account.desc().content().right().clone(),
                device.desc().public_key().clone(),
            )),
            // SavedMetaObject::People(people) => {
            //     public_key_list.push((union_account.desc().content().right().clone(), people.desc()))
            // }
            _ => {
                return Err(crate::meta_err!(ERROR_RIGHT_ACCOUNT_TYPE));
            }
        }

        if !tx.verify(public_key_list)? {
            return Err(crate::meta_err!(ERROR_INVALID));
        }

        let account_id = tx.body.account.desc().calculate_id();
        context
            .ref_state()
            .to_rc()?
            .create_obj_desc(
                &account_id,
                &SavedMetaObject::try_from(StandardObject::UnionAccount(tx.body.account.clone()))?,
            )
            .await?;

        //转账
        if tx.body.left_balance > 0 {
            self.ref_state
                .to_rc()?
                .dec_balance(
                    &tx.body.ctid,
                    &union_account.desc().content().left(),
                    tx.body.left_balance,
                )
                .await?;
            let account_state =
                UnionAccountState::new(union_account.desc(), &self.ref_state.to_rc()?)?;
            account_state
                .deposit(
                    fee_counter,
                    &union_account.desc().content().left(),
                    &tx.body.ctid,
                    tx.body.left_balance,
                )
                .await?;
        }
        if tx.body.right_balance > 0 {
            self.ref_state
                .to_rc()?
                .dec_balance(
                    &tx.body.ctid,
                    &union_account.desc().content().right(),
                    tx.body.right_balance,
                )
                .await?;
            let account_state =
                UnionAccountState::new(union_account.desc(), &self.ref_state.to_rc()?)?;
            account_state
                .deposit(
                    fee_counter,
                    &union_account.desc().content().right(),
                    &tx.body.ctid,
                    tx.body.right_balance,
                )
                .await?;
        }
        Ok(())
    }

    pub async fn execute_deviate_union(
        &self,
        context: &mut ExecuteContext,
        fee_counter: &mut context::FeeCounter,
        tx: &DeviateUnionTx,
    ) -> BuckyResult<()> {
        //获取联合账户信息
        let ret = context
            .ref_state()
            .to_rc()?
            .get_obj_desc(&tx.body.union)
            .await?;
        let union_account = if let SavedMetaObject::UnionAccount(union_account) = ret {
            union_account
        } else {
            return Err(crate::meta_err!(ERROR_DESC_TYPE));
        };

        let mut public_key_list = Vec::new();
        let ret = context
            .ref_state()
            .to_rc()?
            .get_obj_desc(&union_account.desc().content().left())
            .await;
        if let Err(err) = &ret {
            let err_code = get_meta_err_code(err)?;
            return if err_code == ERROR_NOT_FOUND {
                Err(crate::meta_err!(ERROR_CANT_FIND_LEFT_USER_DESC))
            } else {
                Err(crate::meta_err2!(err_code, err.msg()))
            };
        }
        let left_account = ret.unwrap();
        match left_account {
            SavedMetaObject::Device(device) => public_key_list.push((
                union_account.desc().content().left().clone(),
                device.desc().public_key().clone(),
            )),
            // SavedMetaObject::People(people) => {
            //     public_key_list.push((union_account.desc().content().right().clone(), people.desc()))
            // }
            _ => return Err(crate::meta_err!(ERROR_LEFT_ACCOUNT_TYPE)),
        }

        let ret = context
            .ref_state()
            .to_rc()?
            .get_obj_desc(&union_account.desc().content().right())
            .await;
        if let Err(err) = &ret {
            let err_code = get_meta_err_code(err)?;
            return if err_code == ERROR_NOT_FOUND {
                Err(crate::meta_err!(ERROR_CANT_FIND_RIGHT_USER_DESC))
            } else {
                Err(crate::meta_err2!(err_code, err.msg()))
            };
        }
        let right_account = ret.unwrap();
        match right_account {
            SavedMetaObject::Device(device) => public_key_list.push((
                union_account.desc().content().right().clone(),
                device.desc().public_key().clone(),
            )),
            // SavedMetaObject::People(people) => {
            //     public_key_list.push((union_account.desc().content().right().clone(), people.desc()))
            // }
            _ => {
                return Err(crate::meta_err!(ERROR_RIGHT_ACCOUNT_TYPE));
            }
        }

        if !tx.verify(public_key_list)? {
            return Err(crate::meta_err!(ERROR_INVALID));
        }

        let account_state = UnionAccountState::new(union_account.desc(), &self.ref_state.to_rc()?)?;
        account_state
            .deviate(fee_counter, &tx.body.ctid, tx.body.deviation, tx.body.seq)
            .await?;

        Ok(())
    }

    pub async fn execute_withdraw_from_union(
        &self,
        context: &mut ExecuteContext,
        _fee_counter: &mut context::FeeCounter,
        tx: &WithdrawFromUnionTx,
    ) -> BuckyResult<()> {
        if let context::AccountMethods::Single(_) = context.caller().methods() {
            //获取联合账户信息
            let ret = context.ref_state().to_rc()?.get_obj_desc(&tx.union).await?;
            let union_account = if let SavedMetaObject::UnionAccount(union_account) = ret {
                union_account
            } else {
                return Err(crate::meta_err!(ERROR_DESC_TYPE));
            };

            let caller_id = context.caller().id().clone();
            if union_account.desc().content().right() != &caller_id
                && union_account.desc().content().left() != &caller_id
            {
                return Err(crate::meta_err!(ERROR_ACCESS_DENIED));
            }

            self.union_withdraw_manager
                .to_rc()?
                .withdraw(context.block(), &tx.union, &caller_id, &tx.ctid, tx.value)
                .await
            // context::UnionAccountState::load(&tx.union
            //                                  , &context.ref_state().to_rc()?)?
            //     .withdraw(
            //               , context.caller().id()
            //               , &tx.ctid)
        } else {
            Err(crate::meta_err!(ERROR_INVALID))
        }
    }

    // 检查caller是否有对应target的权限：
    // 1. caller就是target本身，有权限
    // 2. caller是target的某一层owner，有权限
    // 否则都算无权限
    async fn check_permission(
        context: &mut ExecuteContext,
        target: &ObjectId,
    ) -> BuckyResult<bool> {
        let caller = context.caller().id().clone();
        let mut check = target.clone();
        loop {
            if caller == check {
                return Ok(true);
            }

            let desc = context.ref_state().to_rc()?.get_obj_desc(target).await?;
            // 尝试把target_data转成AnyNamedObject
            let obj = AnyNamedObject::try_from(desc)?;
            // 一直找到没有owner的对象为止
            if obj.owner().is_none() {
                return Ok(false);
            }
            check = obj.owner().as_ref().unwrap().clone();
        }
    }

    pub async fn execute_set_benefi_tx(
        &self,
        context: &mut ExecuteContext,
        _fee_counter: &mut context::FeeCounter,
        tx: &SetBenefiTx,
    ) -> BuckyResult<()> {
        // 获取address现在的受益人信息, 没有受益人的情况，benefi是address自己
        let benefi = context
            .ref_state()
            .to_rc()?
            .get_beneficiary(&tx.address)
            .await?;

        // 检查caller是不是受益人，或者受益人的owner
        let mut permission = Self::check_permission(context, &benefi).await?;

        // 这里加一个逻辑：原来address的owner也有权限修改受益人，临时对应受益人为合约，不能再转走的问题
        if !permission {
            permission = Self::check_permission(context, &tx.address).await?;
        }

        if !permission {
            return Err(BuckyError::new(
                BuckyErrorCode::PermissionDenied,
                "caller is not benefi or benefi`s owner",
            ));
        }

        // 修改benefi
        context
            .ref_state()
            .to_rc()?
            .set_beneficiary(&tx.address, &tx.to)
            .await?;
        Ok(())
    }
}

//Ok(feed),Error(errno)
//check函数的实现不依赖state模块 (只要有必要的meta config,就可以
/*
pub fn check_trans_tx(tx:&MetaTx, trans_body:&TransTokenTx, op_desc:&Desc) -> Result<u32,u32> {
    //1) 基础检测
    let feed = check_tx_feed(tx)?;

    //2) 业务相关检测
    if let Some(from_id) = &trans_body.from {
        if *from_id == tx.opid {
            return Err(ERROR_SAME_FROM_OP);
        }
    }

    for (to_id,v) in &trans_body.to {
        if *v == 0 {
            return Err(ERROR_TO_IS_ZERO);
        }
    }


    return Ok(feed);
}


pub fn proc_trans_tx(tx:&MetaTx, tx_body:&TransTokenTx, feed:u64, meta_db:&mut MetaDBClient) ->Result<(),u32> {
    /*
    转账的业务逻辑
    1.opid和from可以不是一个人(op操作from账号).如果是同一个人，则tx_body的from可以不填
    2.feed一是从opid上扣
    3.为了提升系统匿名性的支持，opid对应的obj-desc可以没被创建过，这时需要通过op_desc里的信息来确定opid有足够的签名来操作
      没有通过creat_desc操作创建的object,可以有“objid余额”，但在使用其余额之前，这个object可以认为是不存在的，
    4.调用account_instance的sub接口尝试扣款,手续费和金额都必须成功
    5.调用to_instance的add接口完成转入操作。
    6.任何account_instance在首次使用（或实例化desc）时如果还有账户余额，那么实质上都是创建了object.
    */

    //先判断tx是否有足够的签名来操作from
    let mut instance;
    let mut is_op_not_from = false;
    let mut db_transaction = meta_db.start_transaction().unwrap();
    if let Some(fid) = &tx_body.from {
        is_op_not_from = true;
        instance = get_account_instance(fid,&None);

    } else {
        instance = get_account_instance(&tx.opid,&tx.op_desc);
    }

    match instance {
        Ok(account_instance) => {
            if !tx_body.token_id.is_contract() {
                let coin_id = tx_body.token_id.get_coin_id();
                let mut total:u64 = 0;
                for (to_id,v) in &tx_body.to {
                    total = total + *v;
                }

                if is_op_not_from {
                    let opi = get_account_instance(&tx.opid,&tx.op_desc);
                    if let Ok(opi) = &opi {
                        let coin_token_id = ObjectId::from_coin_id(tx.gas_coin_id);
                        opi.trans_sub_balance(feed, &tx.opid, &tx.ext_sig, None, &coin_token_id, meta_db);
                    } else {
                        log::warn!("cann't find op account,op={}",tx.opid.to_string());
                        return Err(ERROR_NOT_FOUND);
                    }
                } else {
                    total = total + feed;
                    //meta_db.start_transaction();
                }
                //先扣款 先不需要await?
                //TODO:不能这么扣，feed的to不同
                account_instance.trans_sub_balance(total, &tx.opid, &tx.ext_sig,Some(&tx_body.to),&tx_body.token_id, meta_db);//to_instance的接口
                //再转入
                for (to_id,v) in &tx_body.to {
                    let to_instance = get_account_instance(&to_id,&None);
                    match to_instance {
                        Ok(to_account) => {
                            to_account.trans_add_balance(*v, &tx.opid, &tx_body.token_id, meta_db);//to_instance的接口
                        },
                        Err(errno) => {
                            log::warn!("tx.to account not found,proc trans tx error.{}",to_id.to_string());
                            return Ok(());
                        }
                    }

                }
                db_transaction.commit();
            } else {
                //TODO:先简单处理，如果涉及到两个contract交叉，会存在效率问题
                // 这里也涉及到如何定义contract的接口的问题
                // 搞定正确性再关注性能。

                //let contract_instance = get_contract_instance(&TransTokenTx.token_id);
                //contract_instance.proc_trans_tx(tx,TransTokenTx,meta_db);
            }
        },
        Err(errno) => {
            log::warn!("opid account not found,proc trans tx error");
            return Err(ERROR_NOT_FOUND);
        }
    }

    return Ok(());
}



pub fn check_set_union_tx(tx:&MetaTx,set_union_body:&SetUnionAccountBody,op_desc:&UnionAccountDesc) -> Result<u32,u32> {
    //1) 基础检测
    let feed = check_tx_feed(tx)?;

    //2) 业务相关检查 ,如果is_return设置了，就必须设置 condition
    //               有多签名
    if set_union_body.is_return {
        if let None = tx.condition {
            return Err(ERROR_NEED_CONDITION);
        }
    } else {
        if tx.ext_sig.len() != 1 {
            return Err(ERROR_ACCESS_DENIED);
        }

        //TODO：检查是否有正确的双签名
        if set_union_body.op_is_left {} else {}
    }

    return Ok(feed);
}

pub fn proc_set_union_tx(tx:&MetaTx, account_desc:&UnionAccountDesc, set_union_body:&SetUnionAccountBody, feed:u64, meta_db:&mut MetaDBClient) -> Result<(),u32> {
    if let Ok(mut ua_instance) = get_union_account_instance(&tx.opid) {
        if set_union_body.is_return {
            //TODO:这里要确认是因为条件满足进来的pending proc
            let mut left_instance;
            let mut right_instance;
            let mut db_transaction = meta_db.start_transaction().unwrap();
            if let Ok(li) = get_account_instance(&account_desc.left,&None) {
                left_instance = li;
            } else {
                log::warn!("union account.left({}) not found,proc_set_union_tx  error",account_desc.left.to_string());
                return Err(ERROR_NOT_FOUND);
            }

            if let Ok(ri) = get_account_instance(&account_desc.right,&None) {
                right_instance = ri;
            } else {
                log::warn!("union account.right({}) not found,proc_set_union_tx  error",account_desc.right.to_string());
                return Err(ERROR_NOT_FOUND);
            }

            //meta_db.start_transaction();
            let coin_token_id = ObjectId::from_coin_id(tx.gas_coin_id);
            if set_union_body.op_is_left {
                left_instance.trans_sub_balance(feed, &account_desc.left,&tx.ext_sig, None,&coin_token_id, &meta_db);
            } else {
                right_instance.trans_sub_balance(feed, &account_desc.right,&tx.ext_sig,None, &coin_token_id, &meta_db);
            }
            left_instance.trans_add_balance(ua_instance.v_left, &tx.opid, &set_union_body.token_id, &meta_db);
            right_instance.trans_add_balance(ua_instance.v_right, &tx.opid, &set_union_body.token_id, &meta_db);
            db_transaction.commit();

        } else {
            if set_union_body.seq > ua_instance.seq {
                if ua_instance.v_right + ua_instance.v_left == set_union_body.left + set_union_body.right {
                    //OK
                    let op_id;
                    if set_union_body.op_is_left {
                        op_id = &account_desc.left;
                    } else {
                        op_id = &account_desc.right;
                    }

                    if let Ok(op_instance) = get_account_instance(op_id,&None) {
                        let mut db_t = meta_db.start_transaction().unwrap();
                        let coin_token_id = ObjectId::from_coin_id(tx.gas_coin_id);
                        op_instance.trans_sub_balance(feed, op_id, &tx.ext_sig,None,&coin_token_id, &meta_db);
                        ua_instance.set(set_union_body.seq,set_union_body.left,set_union_body.right,&meta_db);
                        db_t.commit();
                        return Ok(());
                    } else {
                        log::warn!("cann't find opid {}",op_id.to_string());
                        return Err(ERROR_NOT_FOUND);
                    }
                } else {
                    log::warn!("new union account balance error!");
                    return Err(ERROR_TOTAL_BALANCE);
                }
            } else {
                log::warn!("set op's seq is to small!");
                return Err(ERROR_TOO_SMALL_SEQ);
            }
        }
    } else {
        log::warn!("cann't find union account.");
        return Err(ERROR_NOT_FOUND);
    }


    return Ok(());
}
 */

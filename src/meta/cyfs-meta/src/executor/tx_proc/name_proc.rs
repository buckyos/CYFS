use cyfs_base::*;
//use cyfs_base_meta::{TransBalanceTx, DeviateUnionTx, WithdrawFromUnionTx};

use crate::executor::context;
use crate::executor::transaction::ExecuteContext;
use crate::executor::tx_executor::TxExecutor;
use crate::helper::{get_meta_err_code, ArcWeakHelper};
use std::i64::MAX;
use cyfs_base_meta::*;
use crate::State;

/*
pub fn check_pay_name_tx(tx:&MetaTx, tx_body:&BidNameTx) -> Result<u32,u32> {
    let feed = check_tx_feed(tx)?;
    if let Some(owner_id) = tx_body.owner {
        if owner_id == tx.opid {
            return Err(ERROR_OP_IS_OWNER);
        }
    }

    if tx_body.name_price < MetaConfig::get_min_name_buy_price(tx_body.name.as_str()) {
        return Err(ERROR_NAME_BUY_PRICE);
    }

    let coin_id = MetaConfig::get_buy_name_coin_id();
    if tx_body.price < MetaConfig::get_min_name_rent_price(coin_id) {
        return Err(ERROR_NAME_RENT_PRICE);
    }

    return Ok(feed);
}
*/


fn check_name_state(state: &NameState) -> BuckyResult<()> {
    match state {
        NameState::Normal => {Ok(())},
        _ => {Err(crate::meta_err!(ERROR_ACCESS_DENIED))},
    }
}

impl TxExecutor {
    //TODO: meta-chain需要有一个timer来刷所有name的状态（和扣租金是一个状态）
    pub async fn execute_bid_name_tx(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, tx: &BidNameTx) -> BuckyResult<()>{
        let ownerid: &ObjectId;
        if tx.owner.is_none() {
            ownerid = context.caller().id();
        } else {
            ownerid = tx.owner.as_ref().unwrap();
        }

        let new_info = NameInfo {
            sub_records: Default::default(),
            record: NameRecord {
                link: NameLink::ObjectLink(ownerid.clone()),
                user_data: "".to_string()
            },
            owner: None,
        };

        let ret = context.ref_state().to_rc()?.create_name_info(tx.name.as_str(),&new_info).await;
        if let Err(err) = &ret {
            let errcode = get_meta_err_code(err)?;
            if errcode != ERROR_ALREADY_EXIST {
                return ret;
            }
        }

        let bid_id = context.caller().id().clone();
        self.auction.to_rc()?.bid_name(context.block(), tx.name.as_str(), &bid_id
                              , context.config().to_rc()?.buy_name_coin_id(tx.name.as_str()), tx.name_price as i64, tx.price as i64).await?;

        // 添加一个ChangeName的Event，测试用，先写死1块
        // let event_key = format!("change_{}", &tx.name);
        // let execute_height = context.block().number()+1;
        // context.ref_state().to_rc()?.add_or_update_event(&event_key, Event::ChangeNameEvent(ChangeNameParam{
        //     name: tx.name.clone(),
        //     to: NameState::Normal
        // }), execute_height)?;

        return Ok(());

        /*
        let mut db_t = meta_db.start_transaction().unwrap();
        let op_instance = get_account_instance(&tx.opid,&tx.op_desc);

        if let Ok(opi) = op_instance {
            let coin_obj_id = ObjectId::from_coin_id(tx.gas_coin_id);
            opi.trans_sub_balance(feed,&tx.opid,&tx.ext_sig,None,&coin_obj_id,meta_db)?;
        } else {
            log::warn!("cann't proc tx pay_name,op not found.tx:{}",tx.get_debug_string());
            return Err(ERROR_OP_NOT_FOUND);
        }

        let name_info = get_name_info(&tx_body.name,meta_db)?;
        //TODO:一定不会出错？ 这里的事务性怎么保障？（不能返回多个new状态？）

        match name_info.state {
            FFSNameState::Normal=>{
                //TODO:续费操作？
                log::warn!("cann't buy normal NAME,tx:{}",tx.get_debug_string());
                return Err(ERROR_NAME_STATE_ERROR);
            },
            FFSNameState::Lock => {
                //TODO:续费操作？ 充值足够则让name的状态重新回到Normal

                return Err(ERROR_NAME_STATE_ERROR);
            },
            FFSNameState::New=>{
                //得到该name的管理帐号，判断是否允许购买该name

                //将value转入name
                //状态更新为AuctionNew
                //转入足够的余额，剩下的出价则转入 MetaChain的Name出售帐号（根据name的后缀）
            },
            FFSNameState::AuctionNew=>{
                //判断出价是否合适
                //看上一个owner是否自己，如果不是自己则获得owner
                //  讲Name中的余额回退给之前的owner(返还拍卖资金)
                //如果是自己，则查看上次出价的时间
                //  到现在的时间没有落锤，则更新价格
                //  否则直接成交，修改状态为Normal
            },
            FFSNameState::AuctionOut=>{
                //规则同上？
            },
            FFSNameState::AuctionOut2=>{
                //规则同上？
            },
        };


        db_t.commit();
        */

    }

    pub async fn execute_auction_name_tx(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, tx: &AuctionNameTx) -> BuckyResult<()> {
        if let Some((name_info,_)) = context.ref_state().to_rc()?.get_name_info(tx.name.as_str()).await? {
            if name_info.owner.is_some() && name_info.owner.unwrap().eq(context.caller().id()) {
                self.auction.to_rc()?.active_auction_name(tx.name.as_str(), MAX, tx.price as i64).await
            } else {
                Err(crate::meta_err!(ERROR_ACCESS_DENIED))
            }
        } else {
            return Err(crate::meta_err!(ERROR_NOT_FOUND))
        }
    }

    pub async fn execute_cancel_auction_name_tx(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, tx: &CancelAuctionNameTx) -> BuckyResult<()> {
        if let Some((name_info,_)) = context.ref_state().to_rc()?.get_name_info(tx.name.as_str()).await? {
            if name_info.owner.is_some() && name_info.owner.unwrap().eq(context.caller().id()) {
                self.auction.to_rc()?.cancel_auction(context.block(), tx.name.as_str()).await
            } else {
                Err(crate::meta_err!(ERROR_ACCESS_DENIED))
            }
        } else {
            return Err(crate::meta_err!(ERROR_NOT_FOUND))
        }
    }

    pub async fn execute_buy_back_name_tx(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, tx: &BuyBackNameTx) -> BuckyResult<()> {
        if let Some((name_info,_)) = context.ref_state().to_rc()?.get_name_info(tx.name.as_str()).await? {
            let caller_id = context.caller().id().clone();
            if name_info.owner.is_some() && name_info.owner.unwrap().eq(&caller_id) {
                self.auction.to_rc()?.buy_back_name(context.block(), tx.name.as_str(), &caller_id).await
            } else {
                Err(crate::meta_err!(ERROR_ACCESS_DENIED))
            }
        } else {
            return Err(crate::meta_err!(ERROR_NOT_FOUND))
        }
    }

    pub async fn execute_update_name_info_tx(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, tx: &UpdateNameTx) -> BuckyResult<()>{
        if let Some((name_info,name_state)) = context.ref_state().to_rc()?.get_name_info(tx.name.as_str()).await?{
            check_name_state(&name_state)?;
            if name_info.owner.is_some() && name_info.owner.unwrap().eq(context.caller().id()) {
                context.ref_state().to_rc()?.update_name_info(tx.name.as_str(),&tx.info).await?;
            } else {
                return Err(crate::meta_err!(ERROR_ACCESS_DENIED));
            }
        } else {
            return Err(crate::meta_err!(ERROR_NOT_FOUND));
        }
        return Ok(());
    }
}
/*
pub fn proc_change_name_owner_tx() {
    //得到name的状态
    //如果是个已经存在的normal name
    //    判断opid是否有足够的权限-> 修改owner
    //如果是new name
    //    判断name是否是sub name，得到parent name
    //    判断opid是否有权限再parent name之下建立新的sub name
    //    直接创建新的 sub name

    unimplemented!();
}

pub fn check_change_name_state_tx(tx:&MetaTx,tx_body:&ChangeNameStateTx) -> Result<u32,u32> {
    unimplemented!();
}

pub fn proc_change_name_state_tx() {
    //得到name的状态
    //如果是normal状态
    // 判断opid是否有足够的权限修改name的状态
    // 执行状态修改，回退name中的余额

    unimplemented!();
}
 */

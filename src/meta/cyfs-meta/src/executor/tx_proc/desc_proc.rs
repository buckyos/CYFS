use crate::executor::context;
use crate::executor::transaction::ExecuteContext;
use crate::executor::tx_executor::TxExecutor;
use crate::helper::ArcWeakHelper;
use crate::*;
use cyfs_base::*;
use log::*;

/*
pub fn check_trans_and_create_desc_tx(tx:&MetaTx,tx_body:&CreateDescTx) -> Result<u32,u32> {
    let feed = check_tx_feed(tx)?;

    if tx_body.price < MetaConfig::get_min_desc_price(tx_body.coin_id) {
        return Err(ERROR_DESC_PRICE);
    }

    let desc_len = 100; //TODO:要算desc的长度
    if tx_body.value < tx_body.price as u64 * desc_len * 30 {
        return Err(ERROR_DESC_VALUE);
    }

    return Ok(feed);
}*/

/*
一个典型的对象创建流程

*/

impl TxExecutor {
    pub async fn execute_trans_and_create_desc_tx(
        &self,
        context: &mut ExecuteContext,
        fee_counter: &mut context::FeeCounter,
        tx: &CreateDescTx,
        meta_tx: &MetaTx,
    ) -> BuckyResult<()> {
        //1.判断desc是否已经存在
        //2.完成op操作from往to转账的操作,并有足够的手续费
        //3.根据to的desc创建to对象,注意to的ObjectId对链来说必须是个id，如果id已经存在（有余额）那么该操作会失败，应该使用UpdateDescTx来创建Object
        //  TODO:3背后的逻辑比较复杂，请参考文档
        //  注意“自创建逻辑”
        //4.手续费目前是tx上链成功才扣，我们不打算在blockchain上保存失败的tx

        let body = meta_tx.body();
        if body.is_none() {
            return Err(meta_err!(ERROR_NO_BODY_DATA));
        }

        let data = &body.as_ref().unwrap().content().data;
        let ret = SavedMetaObject::clone_from_slice(data.as_slice());
        if ret.is_err() {
            return Err(meta_err!(ERROR_PARSE_BODY_FAILED));
        }

        let desc = ret.unwrap();
        if desc.hash()? != tx.desc_hash {
            error!("data hash mismatch!, except {}, actual {}", &tx.desc_hash, desc.hash()?);
            error!("tx data hex: {}", hex::encode(data));
            error!("rust data hex: {}", hex::encode(desc.to_vec().unwrap()));
            return Err(meta_err!(ERROR_HASH_ERROR));
        }

        let objid = context::id_from_desc(&desc);

        //TODO:是否需要检测已经objid 已经有余额存在?
        //     是否需要校验caller能否创建desc?
        context
            .ref_state()
            .to_rc()?
            .create_obj_desc(&objid, &desc)
            .await?;

        if let SavedMetaObject::File(file) = &desc {
            if file.desc().owner().is_some() {
                context.ref_state().to_rc()?.set_beneficiary(&file.desc().calculate_id(), file.desc().owner().as_ref().unwrap()).await?;
            }
        } else if let SavedMetaObject::Data(data) = &desc {
            if let Ok(obj) = AnyNamedObject::clone_from_slice(data.data.as_slice()) {
                if obj.owner().is_some() {
                    context.ref_state().to_rc()?.set_beneficiary(&obj.calculate_id(), obj.owner().as_ref().unwrap()).await?;
                }
            }
        } else if let SavedMetaObject::Device(device) = &desc {
            if device.desc().owner().is_some() {
                context.ref_state().to_rc()?.set_beneficiary(&device.desc().calculate_id(), device.desc().owner().as_ref().unwrap()).await?;
            }
        }

        self.rent_manager
            .to_rc()?
            .add_rent_desc(
                context.block(),
                &objid,
                tx.coin_id,
                tx.price as i64,
                desc.raw_measure(&None)? as i64,
            )
            .await?;

        //对象创建成功，给新对象转账
        if let context::AccountMethods::Single(methods) = context.caller().methods() {
            let to = vec![(objid, tx.value)];
            methods
                .trans_balance(fee_counter, &CoinTokenId::Coin(tx.coin_id), &to)
                .await?;
        } else {
            return Err(crate::meta_err!(ERROR_INVALID));
        }

        return Ok(());
    }

    /*
    pub fn check_update_desc_tx(tx:&MetaTx,tx_body:&UpdateDescTx) -> Result<u32,u32> {
        let feed = check_tx_feed(tx)?;

        if let Some(price) = &tx_body.price {
            if price.price < MetaConfig::get_min_desc_price(price.coin_id) {
                return Err(ERROR_DESC_PRICE);
            }
        }

        return Ok(feed);
    }
    */

    pub async fn execute_update_desc_tx(
        &self,
        context: &mut ExecuteContext,
        _fee_counter: &mut context::FeeCounter,
        tx: &UpdateDescTx,
        meta_tx: &MetaTx,
    ) -> BuckyResult<()> {
        let body = meta_tx.body();
        if body.is_none() {
            return Err(meta_err!(ERROR_NO_BODY_DATA));
        }

        let data = &body.as_ref().unwrap().content().data;
        let ret = SavedMetaObject::clone_from_slice(data.as_slice());
        if ret.is_err() {
            return Err(meta_err!(ERROR_PARSE_BODY_FAILED));
        }

        let desc = ret.unwrap();
        if desc.hash()? != tx.desc_hash {
            return Err(meta_err!(ERROR_HASH_ERROR));
        }

        let objid = context::id_from_desc(&desc);

        //TODO:判断caller是否有权限update desc
        //if objid != context.caller().id() {
        //    return Err(ERROR_ACCESS_DENIED);
        //}

        let mut rent_state = context.ref_state().to_rc()?.get_desc_extra(&objid).await?;
        if rent_state.rent_arrears > 0 {
            return Err(crate::meta_err!(ERROR_RENT_ARREARS));
        }

        context
            .ref_state()
            .to_rc()?
            .update_obj_desc(&objid, &desc, 0)
            .await?;

        if let Some(price) = &tx.price {
            if rent_state.coin_id != price.coin_id || rent_state.rent_value != price.price as i64 {
                rent_state.coin_id = price.coin_id;
                rent_state.rent_value = price.price as i64;
            }
        }

        rent_state.data_len = desc.raw_measure(&None)? as i64;
        context
            .ref_state()
            .to_rc()?
            .update_desc_extra(&rent_state)
            .await?;

        /*
        let obj_id = ObjectId::new();//TODO:从Desc中计算得到
        let mut db_t = meta_db.start_transaction().unwrap();
        //判断desc是否已经存在
        let mut obj_desc = match get_object(&obj_id,meta_db) {
            Ok(r) => r,
            Err(errno) => {
                log::warn!("object desc not found,cann't update,tx:{}",tx.get_debug_string());
                return Err(errno);
            },
        };

        //如果obj_desc已经欠费，则需要先充值才可以继续,
        match obj_desc.get_state() {
            FFSObjectState::Normal => (),
            FFSObjectState::Expire => {
                log::warn!("object state is not normal, cann't update desc,tx:{}",tx.get_debug_string());
                return Err(ERROR_DESC_STATE_NOT_NORMAL);
            }
        };

        let op_account =  match get_account_instance(&tx.opid, &tx.op_desc) {
            Ok(r) => r,
            Err(errono) => {
                log::warn!("can't instance op account tx:{}",tx.get_debug_string());
                return Err(ERROR_OP_NOT_FOUND);
            }
        };

        let coin_id = ObjectId::from_coin_id(tx.gas_coin_id);
        op_account.trans_sub_balance(feed,&tx.opid,&tx.ext_sig,None,&coin_id,meta_db)?;

        obj_desc.update_desc(&tx.opid,&tx.ext_sig,&tx_body.desc,tx_body.write_flag,&tx_body.price)?;

        db_t.commit();


        */
        return Ok(());
    }

    pub async fn execute_remove_desc_tx(
        &self,
        _context: &mut ExecuteContext,
        _fee_counter: &mut context::FeeCounter,
        _tx: &RemoveDescTx,
    ) -> BuckyResult<()> {
        let desc_obj = self.ref_state.to_rc()?.get_obj_desc(&_tx.id).await?;
        if let SavedMetaObject::File(file) = desc_obj {
            //TODO:判断caller是否有权限remove desc
            if file.desc().owner().as_ref().unwrap() != _context.caller().id() {
                return Err(crate::meta_err!(ERROR_ACCESS_DENIED));
            }
            let desc_extra = _context
                .ref_state()
                .to_rc()?
                .get_desc_extra(&_tx.id)
                .await?;
            if desc_extra.other_charge_balance > 0 {
                return Err(crate::meta_err!(ERROR_OTHER_CHARGED));
            }
            let balance = _context
                .ref_state()
                .to_rc()?
                .get_balance(&_tx.id, &CoinTokenId::Coin(desc_extra.coin_id))
                .await?;
            if balance > 0 {
                _context
                    .ref_state()
                    .to_rc()?
                    .inc_balance(
                        &CoinTokenId::Coin(desc_extra.coin_id),
                        file.desc().owner().as_ref().unwrap(),
                        balance,
                    )
                    .await?;
                _context
                    .ref_state()
                    .to_rc()?
                    .dec_balance(&CoinTokenId::Coin(desc_extra.coin_id), &_tx.id, balance)
                    .await?;
            }
        } else {
            //TODO:判断caller是否有权限remove desc
            if &_tx.id != _context.caller().id() {
                return Err(crate::meta_err!(ERROR_ACCESS_DENIED));
            }
        }
        _context.ref_state().to_rc()?.drop_desc(&_tx.id).await?;
        self.rent_manager.to_rc()?.delete_rent_desc(&_tx.id).await?;
        Ok(())
    }
}

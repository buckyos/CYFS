use cyfs_base::*;
use cyfs_base_meta::*;
use crate::state_storage::{StateRef, StateWeakRef};
use super::FeeCounter;
use log::*;
use crate::helper::{get_meta_err_code, ArcWeakHelper};
use crate::events::event_manager::{EventManagerWeakRef, EventManagerRef};
use crate::executor::context::{ConfigWeakRef, ConfigRef};
use crate::State;
use std::sync::{Arc, Weak};

pub struct UnionAccountState {
    id: ObjectId,
    union: UnionAccountDesc,
    ref_state: StateWeakRef
}

impl UnionAccountState {
    pub fn new(desc: &UnionAccountDesc, ref_state: &StateRef) -> BuckyResult<UnionAccountState> {
        Ok(UnionAccountState {
            id: desc.calculate_id(),
            union: desc.clone(),
            ref_state: StateRef::downgrade(ref_state)
        })
    }


    // load id from state
    pub fn load(id: &ObjectId, ref_state: &StateRef) -> BuckyResult<UnionAccountState> {
        info!("load union state");
        let desc = async_std::task::block_on(async {ref_state.get_obj_desc(id).await})?;
        if let SavedMetaObject::UnionAccount(union) = desc {
            UnionAccountState::new(union.desc(), ref_state)
        } else {
            Err(crate::meta_err!(ERROR_NOT_FOUND))
        }
    }

    pub fn which_peer(&self, peer: &ObjectId) -> Option<PeerOfUnion> {
        if peer ==  self.union.content().left() {
            Some(PeerOfUnion::Left)
        } else if peer ==  self.union.content().right() {
            Some(PeerOfUnion::Right)
        } else {
            None
        }
    }

    pub async fn deposit(&self, fee_counter: &mut FeeCounter, from: &ObjectId, ctid: &CoinTokenId, v: i64) -> BuckyResult<()> {
        let which = self.which_peer(from).ok_or_else(|| crate::meta_err!(ERROR_ACCESS_DENIED))?;
        fee_counter.cost(0)?;
        self.ref_state.to_rc()?.inc_balance(ctid, &self.id, v).await?;
        self.ref_state.to_rc()?.deposit_union_balance(ctid, &self.id, which, v).await?;
        Ok(())
    }

    pub async fn deviate(&self, _fee_counter: &mut FeeCounter, ctid: &CoinTokenId, v: i64, seq: i64) -> BuckyResult<()> {
        self.ref_state.to_rc()?.update_union_deviation(ctid, &self.id, v, seq).await
    }

    pub async fn withdraw(&self, to: &ObjectId, ctid: &CoinTokenId, withdraw: i64) -> BuckyResult<()> {
        let which = self.which_peer(to).ok_or_else(|| crate::meta_err!(ERROR_ACCESS_DENIED))?;
        self.ref_state.to_rc()?.withdraw_union_balance(ctid, &self.id, which, withdraw).await?;
        self.ref_state.to_rc()?.dec_balance(ctid, &self.id, withdraw).await?;
        self.ref_state.to_rc()?.inc_balance(ctid, to, withdraw).await?;
        Ok(())
    }

    pub async fn get_union_balance(&self, ctid: &CoinTokenId) -> BuckyResult<UnionBalance> {
        self.ref_state.to_rc()?.get_union_balance(ctid, &self.id).await
    }

    pub async fn get_deviation_seq(&self, ctid: &CoinTokenId) -> BuckyResult<i64> {
        self.ref_state.to_rc()?.get_union_deviation_seq(ctid, &self.id).await
    }
}

pub type UnionWithdrawManagerRef = Arc<UnionWithdrawManager>;
pub type UnionWithdrawManagerWeakRef = Weak<UnionWithdrawManager>;

pub struct UnionWithdrawManager {
    ref_state: StateWeakRef,
    event_manager: EventManagerWeakRef,
    config: ConfigWeakRef,
}

impl UnionWithdrawManager {
    pub fn new(ref_state: &StateRef, config: &ConfigRef, event_manager: &EventManagerRef) -> UnionWithdrawManagerRef {
        let manager = UnionWithdrawManagerRef::new(UnionWithdrawManager {
            ref_state: StateRef::downgrade(ref_state),
            event_manager: EventManagerRef::downgrade(event_manager),
            config: ConfigRef::downgrade(config),
        });

        let withdraw_manager = UnionWithdrawManagerRef::downgrade(&manager);
        event_manager.register_listener(EventType::UnionWithdraw, move |cur_block: BlockDesc, event: Event| {
            let withdraw_manager = withdraw_manager.clone();
            async move {
                if let Event::UnionWithdraw(param) = &event {
                    withdraw_manager.to_rc()?.on_union_withdraw(&cur_block, &event, &param).await
                } else {
                    Err(crate::meta_err!(ERROR_INVALID))
                }
            }
        });

        manager
    }

    async fn on_union_withdraw(&self, _cur_block: &BlockDesc, _event: &Event, param: &UnionWithdraw) -> BuckyResult<EventResult> {
        let ret = async_std::task::block_on(async {self.ref_state.to_rc()?.get_obj_desc(&param.union_id).await})?;
        let union_account = if let SavedMetaObject::UnionAccount(union_account) = ret {
            union_account
        } else {
            return Err(crate::meta_err!(ERROR_DESC_TYPE));
        };

        let state = UnionAccountState::new(union_account.desc(), &self.ref_state.to_rc()?)?;
        state.withdraw(&param.account_id, &param.ctid, param.value).await?;
        Ok(EventResult::new(0, Vec::new()))
    }

    fn get_withdraw_key(&self, union_id: &ObjectId, account_id: &ObjectId) -> String {
        union_id.to_string() + account_id.to_string().as_str()
    }

    pub async fn withdraw(&self, cur_block: &BlockDesc, union_id: &ObjectId, account_id: &ObjectId, ctid: &CoinTokenId, value: i64) -> BuckyResult<()> {
        let height = cur_block.number() + self.config.to_rc()?.union_withdraw_interval() as i64;
        let event = Event::UnionWithdraw(UnionWithdraw{
            union_id: union_id.clone(),
            account_id: account_id.clone(),
            ctid: ctid.clone(),
            value,
            height,
        });

        let key = self.get_withdraw_key(union_id, account_id);
        self.event_manager.to_rc()?.add_or_update_once_event(key.as_str(), &event, height).await?;

        Ok(())
    }

    pub async fn get_withdraw_event(&self, union: &UnionAccount) -> BuckyResult<Vec<UnionWithdraw>> {
        let union_id = union.desc().calculate_id();
        let left_key = self.get_withdraw_key(&union_id, &union.desc().content().left());
        let right_key = self.get_withdraw_key(&union_id, &union.desc().content().right());

        let mut event_list = Vec::new();

        let event = self.event_manager.to_rc()?.get_once_event(left_key.as_str()).await;
        if let Err(err) = &event {
            let err_code = get_meta_err_code(err)?;
            if err_code != ERROR_NOT_FOUND {
                return Err(BuckyError::new(BuckyErrorCodeEx::MetaError(err_code), err.msg()));
            }
        } else if let Event::UnionWithdraw(param) = event.unwrap() {
            event_list.push(param);
        }

        let event = self.event_manager.to_rc()?.get_once_event(right_key.as_str()).await;
        if let Err(err) = &event {
            let err_code = get_meta_err_code(err)?;
            if err_code != ERROR_NOT_FOUND {
                return Err(BuckyError::new(BuckyErrorCodeEx::MetaError(err_code), err.msg()));
            }
        } else if let Event::UnionWithdraw(param) = event.unwrap() {
            event_list.push(param);
        }

        Ok(event_list)
    }
}

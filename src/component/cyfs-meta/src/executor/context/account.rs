use cyfs_base::*;
use crate::state_storage::{StateRef, StateWeakRef};
use super::SingleAccount;
use super::UnionAccountState;
use crate::helper::{ArcWeakHelper};
use crate::State;


pub struct Account {
    id: ObjectId,
    ref_state: StateWeakRef,
    methods: AccountMethods
}

pub enum AccountMethods {
    Single(SingleAccount),
    Union(UnionAccountState)
}

impl<'a> Account {
    pub fn single(id: &ObjectId, ref_state: &StateRef) -> Account {
        Account {
            id: *id,
            ref_state: StateRef::downgrade(ref_state),
            methods: AccountMethods::Single(SingleAccount::new(*id, ref_state)),
        }
    }

    pub fn from_id(id: &ObjectId, ref_state: &StateRef) -> BuckyResult<Account> {
        fn get_methods<'a>(id: &ObjectId, ref_state: &StateRef) -> BuckyResult<AccountMethods> {
            if id.obj_type_code() == ObjectTypeCode::UnionAccount{
                Ok(AccountMethods::Union(UnionAccountState::load(id, ref_state)?))
            } else {
                Ok(AccountMethods::Single(SingleAccount::new(*id, ref_state)))
            }
        }
        Ok(Account {
            id: *id,
            ref_state: StateRef::downgrade(ref_state),
            methods: get_methods(id, ref_state)?
        })
    }

    pub fn from_caller(caller: &TxCaller, ref_state: &StateRef) -> BuckyResult<Account> {
        fn get_methods(caller: &TxCaller, ref_state: &StateRef) -> BuckyResult<AccountMethods> {
            Ok(match caller {
                TxCaller::People(_) => AccountMethods::Single(SingleAccount::new(caller.id()?, ref_state)),
                TxCaller::Device(_) => AccountMethods::Single(SingleAccount::new(caller.id()?, ref_state)),
                TxCaller::Group(_) => AccountMethods::Single(SingleAccount::new(caller.id()?, ref_state)),
                TxCaller::Union(desc) => AccountMethods::Union(UnionAccountState::new(desc, ref_state)?),
                TxCaller::Miner(id) => AccountMethods::Single(SingleAccount::new(id.clone(), ref_state)),
                TxCaller::Id(id) => AccountMethods::Single(SingleAccount::new(id.clone(), ref_state))
            })
        }
        Ok(Account {
            id: caller.id()?,
            ref_state: StateRef::downgrade(ref_state),
            methods: get_methods(caller, ref_state)?
        })
    }


    pub fn id(&self) -> &ObjectId {
        &self.id
    }

    pub async fn inc_nonce(&mut self) -> BuckyResult<i64> {
        self.ref_state.to_rc()?.inc_nonce(self.id()).await
    }

    pub async fn nonce(&self) -> BuckyResult<i64> {
        self.ref_state.to_rc()?.get_nonce(self.id()).await
    }

    pub async fn balance_of(&self, ctid: &CoinTokenId) -> BuckyResult<i64> {
        self.ref_state.to_rc()?.get_balance(&self.id, ctid).await
    }

    pub fn methods(&self) -> &AccountMethods {
        &self.methods
    }
}

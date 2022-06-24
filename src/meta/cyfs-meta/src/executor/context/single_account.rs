use cyfs_base::*;
use crate::state_storage::{StateRef, StateWeakRef};
use super::UnionAccountState;
use super::FeeCounter;
use crate::helper::{ArcWeakHelper};
use crate::State;
use crate::*;

pub struct SingleAccount {
    id: ObjectId,
    ref_state: StateWeakRef
}


impl<'a> SingleAccount {
    pub fn new(id: ObjectId, ref_state: &StateRef) -> SingleAccount {
        SingleAccount {
            id: id,
            ref_state: StateRef::downgrade(ref_state)
        }
    }

    pub async fn trans_balance(&self, fee_counter: &mut FeeCounter, ctid: &CoinTokenId, to: &Vec<(ObjectId, i64)>, /*backend: &mut MetaBackend, config: &evm::Config*/) -> BuckyResult<Vec<TxLog>> {
        //TODO: Account的转出和转入应该是独立的 合约接口，有合约约束逻辑在
        let tx_logs = vec![];
        for (to_id, v) in to {
            self.ref_state.to_rc()?.dec_balance(ctid, &self.id, *v).await?;
            if ObjectTypeCode::UnionAccount == to_id.obj_type_code() {
                UnionAccountState::load(to_id, &self.ref_state.to_rc()?)?.deposit(fee_counter, &self.id, ctid, *v).await?;
            } else {
                fee_counter.cost(0)?;
                self.ref_state.to_rc()?.inc_balance(ctid, to_id, *v).await?;
                /*
                // 如果to_id是个合约地址，需要evm执行这个合约的代码，触发receive接口
                if let Ok(code) = self.ref_state.to_rc()?.code(to_id).await {
                    let state = MemoryStackState::new(StackSubstateMetadata::new(33000, config), backend);
                    let mut executor = StackExecutor::new(state, config);
                    let (ret, value) = executor.transact_call(self.id.clone(), to_id.clone(), *v as u64, vec![], 33000);
                    if ret.is_succeed() {
                        let (values, logs) = executor.into_state().deconstruct();
                        backend.apply(values, logs, true);
                        backend.move_logs(&mut tx_logs);
                    } else {
                        // 这里返回BuckyError当作错误，阻止转账
                        return Err(BuckyError::from(evm_reason_to_code(ret)));
                    }
                } else {
                    // 是普通地址，走原来的转账逻辑
                    self.ref_state.to_rc()?.inc_balance(ctid, to_id, *v).await?;
                }
                */
            }
        }
        Ok(tx_logs)
    }

    // 这个函数确定to一定不会是合约地址
    /*
    pub async fn trans_balance_raw(&self, fee_counter: &mut FeeCounter, ctid: &CoinTokenId, to: &Vec<(ObjectId, i64)>) -> BuckyResult<()> {
        for (to_id, v) in to {
            self.ref_state.to_rc()?.dec_balance(ctid, &self.id, *v).await?;
            if let ObjectTypeCode::UnionAccount = to_id.obj_type_code().ok_or_else(|| ERROR_ACCESS_DENIED)? {
                UnionAccountState::load(to_id, &self.ref_state.to_rc()?)?.deposit(fee_counter, &self.id, ctid, *v).await?;
            } else {
                fee_counter.cost(0)?;
                // 是普通地址，走原来的转账逻辑
                self.ref_state.to_rc()?.inc_balance(ctid, to_id, *v).await?;
            }
        }
        Ok(())
    }
     */
}

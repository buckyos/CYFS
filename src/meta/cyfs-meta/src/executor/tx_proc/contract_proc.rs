use crate::executor::transaction::ExecuteContext;
use crate::executor::context;
use cyfs_base_meta::*;
use cyfs_base::*;
use crate::executor::tx_executor::TxExecutor;
use crate::helper::{ArcWeakHelper};
use crate::State;
use evm::executor::{StackSubstateMetadata, MemoryStackState, StackExecutor};
use evm::backend::{ApplyBackend};
use primitive_types::H256;
use evm::ExitReason;
use crate::meta_backend::MetaBackend;
use log::*;

/**
    检查传入的objectid是否是管理者id，只有管理者id才可以set config
*/
fn check_admin_caller(_id: &ObjectId) -> bool {
    true
}

impl TxExecutor {
    pub async fn set_config_tx(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, tx: &SetConfigTx) -> BuckyResult<()> {
        if !check_admin_caller(context.caller().id()) {
            return Err(crate::meta_err!(ERROR_ACCESS_DENIED));
        }

        if tx.key == "rent_cycle" {
            if let Ok(cycle) = tx.value.parse::<i64>() {
                self.event_manager.to_rc()?.change_event_cycle(context.block()
                                                               , &vec![EventType::Rent, EventType::NameRent]
                                                               , self.config.to_rc()?.get_rent_cycle()
                                                               , cycle).await?;
            }
        }

        context.ref_state().to_rc()?.config_set(&tx.key, &tx.value).await
    }

    pub async fn execute_create_contract(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, tx: &CreateContractTx, backend: &mut MetaBackend, config: &evm::Config) -> BuckyResult<(ExitReason, Option<ObjectId>, Option<Vec<u8>>, Vec<TxLog>)> {
        // 这里定死100000000 GAS，算是直接送的，不从fee里扣除
        let gas_limit = 100000000;// fee_counter.max_fee() as u64;
        info!("create contract, gas limit {}", gas_limit);
        let state = MemoryStackState::new(StackSubstateMetadata::new(gas_limit, config), backend);
        let mut executor = StackExecutor::new(state, config);
        let (ret, address, value) = executor.transact_create(context.caller().id().clone(), tx.value, tx.init_data.clone(), gas_limit);
        // 扣除gas
        // let used_gas = executor.used_gas();
        // fee_counter.cost(used_gas as u32)?;
        // 如果是Succeed，就apply修改，否则返回错误
        let mut tx_logs = vec![];
        if ret.is_succeed() {
            let (values, logs) = executor.into_state().deconstruct();
            backend.apply(values, logs, true);
            backend.move_logs(&mut tx_logs);
        }

        Ok((ret, address, if value.len() > 0 {Some(value)} else {None}, tx_logs))
    }

    pub async fn execute_create2_contract(&self, context: &mut ExecuteContext, fee_counter: &mut context::FeeCounter, tx: &CreateContract2Tx, backend: &mut MetaBackend, config: &evm::Config) -> BuckyResult<(ExitReason, Option<ObjectId>, Option<Vec<u8>>, Vec<TxLog>)> {
        // 这里定死100000000 GAS，算是直接送的，不从fee里扣除
        let gas_limit = 100000000;// fee_counter.max_fee() as u64;
        let state = MemoryStackState::new(StackSubstateMetadata::new(gas_limit, config), backend);
        let mut executor = StackExecutor::new(state, config);
        let (ret, address, value) = executor.transact_create2(context.caller().id().clone(), tx.value, tx.init_data.clone(), H256::from_slice(&tx.salt), fee_counter.max_fee() as u64);
        // 扣除gas
        // let used_gas = executor.used_gas();
        // fee_counter.cost(used_gas as u32)?;
        // 如果是Succeed，就apply修改，否则返回错误
        let mut tx_logs = vec![];
        if ret.is_succeed() {
            let (values, logs) = executor.into_state().deconstruct();
            backend.apply(values, logs, true);
            backend.move_logs(&mut tx_logs);
        }
        Ok((ret, address, if value.len() > 0 {Some(value)} else {None}, tx_logs))
    }

    pub async fn execute_call_contract(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, tx: &CallContractTx, backend: &mut MetaBackend, config: &evm::Config) -> BuckyResult<(ExitReason, Option<Vec<u8>>, Vec<TxLog>)> {
        // 这里定死10000000 GAS，算是直接送的，不从fee里扣除
        let gas_limit = 10000000;
        info!("call contract, gas limit {}", gas_limit);
        let state = MemoryStackState::new(StackSubstateMetadata::new(gas_limit, config), backend);
        let mut executor = StackExecutor::new(state, config);
        let (ret, value) = executor.transact_call(context.caller().id().clone(), tx.address, tx.value, tx.data.clone(), gas_limit);
        // 扣除gas
        // let used_gas = executor.used_gas();
        // fee_counter.cost(used_gas as u32)?;
        let mut tx_logs = vec![];
        if ret.is_succeed() {
            let (values, logs) = executor.into_state().deconstruct();
            backend.apply(values, logs, true);
            backend.move_logs(&mut tx_logs);
        }
        Ok((ret, if value.len() > 0 {Some(value)} else {None}, tx_logs))
    }
}

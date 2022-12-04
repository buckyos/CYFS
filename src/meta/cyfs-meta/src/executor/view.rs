use cyfs_base_meta::*;
use crate::state_storage::{StateRef, StateWeakRef};
use crate::archive_storage::*;
use super::context;
use crate::{ViewBalanceResult, State};
use crate::executor::context::AccountMethods;
use crate::helper::{ArcWeakHelper};
use cyfs_base::*;
use crate::meta_backend::MetaBackend;
use evm::executor::{MemoryStackState, StackSubstateMetadata, StackExecutor};

struct ViewExecuteContext {

}

impl ViewExecuteContext {

}

pub struct ViewMethodExecutor<M: ViewMethod> {
    method: M,
    ref_state: StateWeakRef,
    ref_archive: ArchiveWeakRef,
    block: BlockDesc,
    evm_config: evm::Config
}

impl <M: ViewMethod> ViewMethodExecutor<M> {
    pub fn new(block: &BlockDesc, ref_state: &StateRef, ref_archive: &ArchiveRef, method: M) -> ViewMethodExecutor<M> {
        ViewMethodExecutor {
            method,
            ref_state: StateRef::downgrade(ref_state),
            ref_archive: ArchiveRef::downgrade(ref_archive),
            block: block.clone(),
            evm_config: evm::Config::istanbul(),    // 先把evm的config创建在这里，以后能自己设置了，应该是外边传进来的
        }
    }
}

impl ViewMethodExecutor<ViewBalanceMethod> {
    pub async fn exec(&self) -> BuckyResult<<ViewBalanceMethod as ViewMethod>::Result> {
        let result;
        let account = context::Account::from_id(&self.method.account, &self.ref_state.to_rc()?)?;
        match account.methods() {
            AccountMethods::Single(_) => {
                let mut vec = vec![];
                for ctid in &self.method.ctid {
                    vec.push((*ctid, account.balance_of(ctid).await?));
                }
                result = ViewBalanceResult::Single(vec)
            },
            AccountMethods::Union(u) => {
                let mut vec = vec![];
                for ctid in &self.method.ctid {
                    vec.push((*ctid, u.get_union_balance(ctid).await?, u.get_deviation_seq(ctid).await?));
                }
                result = ViewBalanceResult::Union(vec)
            },
        }

        Ok(result)
    }
}

impl ViewMethodExecutor<ViewDescMethod> {
    pub async fn exec(&self) -> BuckyResult<<ViewDescMethod as ViewMethod>::Result> {
        self.ref_state.to_rc()?.get_obj_desc(&self.method.id).await
    }
}

impl ViewMethodExecutor<ViewNameMethod> {
    pub async fn exec(&self) -> BuckyResult<<ViewNameMethod as ViewMethod>::Result> {
        self.ref_state.to_rc()?.get_name_info(&self.method.name).await
    }
}

// 查询objects
impl ViewMethodExecutor<ViewRawMethod> {
    pub async fn exec(&self) -> BuckyResult<<ViewRawMethod as ViewMethod>::Result> {
        match self.ref_state.to_rc()?.get_obj_desc(&self.method.id).await {
            Ok(obj) => {
                let status: u8 = 0/* success*/;
                let _ = self.ref_archive.to_rc()?.set_meta_object_stat(&self.method.id, status).await;
                match obj {
                    SavedMetaObject::Device(obj) => Ok(obj.to_vec()?),
                    SavedMetaObject::People(obj) => Ok(obj.to_vec()?),
                    SavedMetaObject::UnionAccount(obj) => Ok(obj.to_vec()?),
                    SavedMetaObject::Group(obj) => Ok(obj.to_vec()?),
                    SavedMetaObject::File(obj) => Ok(obj.to_vec()?),
                    SavedMetaObject::Data(obj) => Ok(obj.data),
                    SavedMetaObject::Org(obj) => Ok(obj.to_vec()?),
                    SavedMetaObject::MinerGroup(obj) => Ok(obj.to_vec()?),
                    SavedMetaObject::SNService(obj) => Ok(obj.to_vec()?),
                    SavedMetaObject::Contract(obj) => Ok(obj.to_vec()?),
                }
            },
            Err(e) => {
                let status: u8 = 1 /* failed */;
                let _ = self.ref_archive.to_rc()?.set_meta_object_stat(&self.method.id, status).await;
                return Err(e);
            },
        }        

    }
}

impl ViewMethodExecutor<ViewContract> {
    pub async fn exec(&self) -> BuckyResult<<ViewContract as ViewMethod>::Result> {
        // 这里应该是只读代码，caller和gas_price都不重要
        let backend = MetaBackend::new(
            &self.ref_state.to_rc()?,
            0,
            &self.block,
            ObjectId::default(),
            None,
            self.evm_config.clone()
        );
        // 这里的gas_limit要怎么设置？为了避免出问题，这里设置一个定值
        let view_gas_limit = 100000;
        let config= evm::Config::istanbul();
        let state = MemoryStackState::new(StackSubstateMetadata::new(view_gas_limit, &config), &backend);
        let mut executor = StackExecutor::new(state, &config);
        let (ret, value) = executor.transact_call(ObjectId::default(), self.method.address, 0, self.method.data.clone(), view_gas_limit);

        Ok(ViewContractResult {
            ret: evm_reason_to_code(ret) as u32,
            value
        })
    }
}

impl ViewMethodExecutor<ViewBenefi> {
    pub async fn exec(&self) -> BuckyResult<<ViewBenefi as ViewMethod>::Result> {
        let benefi = self.ref_state.to_rc()?.get_beneficiary(&self.method.address).await?;
        Ok(ViewBenefiResult{
            address: benefi
        })
    }
}

impl ViewMethodExecutor<ViewLog> {
    pub async fn exec(&self) -> BuckyResult<<ViewLog as ViewMethod>::Result> {
        let logs = self.ref_state.to_rc()?.get_log(&self.method.address, self.method.from, self.method.to, &self.method.topics).await?;
        Ok(ViewLogResult{
            logs
        })
    }
}


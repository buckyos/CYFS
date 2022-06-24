use crate::executor::tx_executor::TxExecutor;
use crate::executor::transaction::ExecuteContext;
use crate::executor::context;
use cyfs_base_meta::{SNServiceTx, SNService, SavedMetaObject, MetaTx};
use cyfs_base::*;
use crate::ArcWeakHelper;
use crate::*;

impl TxExecutor {
    pub async fn execute_sn_tx(&self, tx: &MetaTx, context: &mut ExecuteContext, fee_counter: &mut context::FeeCounter, tx_body: &SNServiceTx) -> BuckyResult<()> {
        match tx_body {
            SNServiceTx::Publish(service) => {
                self.execute_sn_public_tx(context, fee_counter, service).await
            }
            SNServiceTx::Purchase(contract) => {
                self.execute_sn_purchase_tx(tx, context, fee_counter, contract).await
            }
            SNServiceTx::Settle(proof) => {
                self.execute_sn_settle_tx(context, fee_counter, proof).await
            }
            SNServiceTx::Remove(service_id) => {
                context.ref_state().to_rc()?.drop_desc(service_id).await
            }
        }
    }

    async fn execute_sn_public_tx(&self,
                                  context: &mut ExecuteContext,
                                  _fee_counter: &mut context::FeeCounter,
                                  service: &SNService) -> BuckyResult<()> {
        context.ref_state().to_rc()?.create_obj_desc(&service.desc().calculate_id(),
                                                 &SavedMetaObject::SNService(service.clone())).await?;
        Ok(())
    }

    async fn execute_sn_purchase_tx(&self, _tx: &MetaTx, _context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, contract: &Contract) -> BuckyResult<()> {
        let _desc_content = contract.desc().content();
        // if let ContractDescContent::SNContract(sn_contract) = desc_content {
        //     let ret = context.ref_state().to_rc()?.get_obj_desc(&sn_contract.service_id).await;
        //     if let Err(e) = &ret {
        //         return Err(meta_map_err!(e, ERROR_NOT_FOUND, ERROR_CANT_FIND_CONTRACT));
        //     }
        //     let saved_obj = ret.unwrap();
        //     if let SavedMetaObject::SNService(_service) = saved_obj {
        //         let account = sn_contract.account.clone();
        //         context.ref_state().to_rc()?.create_obj_desc(&account.desc().calculate_id(),
        //                                                      &SavedMetaObject::UnionAccount(account)).await?;
        //     }
        //
        //     context.ref_state().to_rc()?.create_obj_desc(&contract.desc().calculate_id(),
        //                                                  &SavedMetaObject::Contract(contract.clone())).await?;
        //     return Ok(());
        // }
        Err(meta_err!(ERROR_UNKNOWN_CONTRACT_TYPE))
    }

    async fn execute_sn_settle_tx(&self, _context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, _tx_body: &ProofOfService) -> BuckyResult<()> {
        // let desc_content = tx_body.desc().content();
        // if let ProofData::ProofOfSNService(_proof) = &desc_content.proof_data {
        //
        //     return Ok(());
        // }
        Err(meta_err!(ERROR_PROOF_TYPE_ERROR))
    }
}

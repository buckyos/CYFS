use crate::executor::tx_executor::TxExecutor;
use crate::executor::transaction::ExecuteContext;
use crate::executor::context;
use cyfs_base::*;
use crate::helper::ArcWeakHelper;
use cyfs_base_meta::BlockDescTrait;
use crate::*;
use crate::AccountInfo;

impl TxExecutor {
    pub async fn execute_create_miners_tx(&self, context: &mut ExecuteContext, _fee_counter: &mut context::FeeCounter, miner_group: &MinerGroup) -> BuckyResult<()> {
        if context.block().number() != 0 {
            return Err(meta_err!(ERROR_GENESIS_MINER_BLOCK_INVALID))
        }

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
        context.ref_state().to_rc()?.config_set("miners_group", miner_group.desc().calculate_id().to_string().as_str()).await?;
        context.ref_state().to_rc()?.add_account_info(&AccountInfo::MinerGroup(miner_group.clone())).await?;
        Ok(())
    }
}

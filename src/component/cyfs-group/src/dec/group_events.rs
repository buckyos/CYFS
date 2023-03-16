use std::sync::Arc;

use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, NamedObject, ObjectDesc, ObjectId,
    ObjectMapIsolatePathOpEnvRef, ObjectMapSingleOpEnvRef, RawConvertTo, RawDecode, RawFrom,
    TypelessCoreObject,
};
use cyfs_core::{GroupConsensusBlock, GroupProposal};
use cyfs_group_lib::{
    ExecuteResult, GroupCommand, GroupCommandCommited, GroupCommandExecute,
    GroupCommandExecuteResult, GroupCommandVerify,
};
use cyfs_lib::NONObjectInfo;

use crate::NONDriverHelper;

#[derive(Clone)]
pub(crate) struct RPathEventNotifier {
    non_driver: NONDriverHelper,
}

impl RPathEventNotifier {
    pub fn new(driver: NONDriverHelper) -> Self {
        Self { non_driver: driver }
    }

    pub async fn on_execute(
        &self,
        proposal: GroupProposal,
        prev_state_id: Option<ObjectId>,
    ) -> BuckyResult<ExecuteResult> {
        let cmd = GroupCommandExecute {
            proposal,
            prev_state_id,
        };

        let cmd = GroupCommand::from(cmd);
        let object_raw_buf = cmd.to_vec()?;
        let any_obj = cyfs_base::AnyNamedObject::Core(TypelessCoreObject::clone_from_slice(
            object_raw_buf.as_slice(),
        )?);

        let result = self
            .non_driver
            .post_object(
                NONObjectInfo {
                    object_id: cmd.desc().object_id(),
                    object_raw: object_raw_buf,
                    object: Some(Arc::new(any_obj)),
                },
                None,
            )
            .await?;

        assert!(result.is_some());
        match result.as_ref() {
            Some(result) => {
                let (cmd, _remain) = GroupCommand::raw_decode(result.object_raw.as_slice())?;
                assert_eq!(_remain.len(), 0);
                let mut cmd = TryInto::<GroupCommandExecuteResult>::try_into(cmd)?;
                Ok(ExecuteResult {
                    result_state_id: cmd.result_state_id.take(),
                    receipt: cmd.receipt.take(),
                    context: cmd.context.take(),
                })
            }
            None => Err(BuckyError::new(
                BuckyErrorCode::Unknown,
                "expect some result from dec-app",
            )),
        }
    }

    pub async fn on_verify(
        &self,
        proposal: GroupProposal,
        prev_state_id: Option<ObjectId>,
        execute_result: &ExecuteResult,
    ) -> BuckyResult<()> {
        let cmd = GroupCommandVerify {
            proposal,
            prev_state_id,
            result_state_id: execute_result.result_state_id.clone(),
            receipt: execute_result.receipt.clone(),
            context: execute_result.context.clone(),
        };

        let cmd = GroupCommand::from(cmd);
        let object_raw_buf = cmd.to_vec()?;
        let any_obj = cyfs_base::AnyNamedObject::Core(TypelessCoreObject::clone_from_slice(
            object_raw_buf.as_slice(),
        )?);

        let result = self
            .non_driver
            .post_object(
                NONObjectInfo {
                    object_id: cmd.desc().object_id(),
                    object_raw: object_raw_buf,
                    object: Some(Arc::new(any_obj)),
                },
                None,
            )
            .await?;

        assert!(result.is_none());
        Ok(())
    }

    pub async fn on_commited(
        &self,
        prev_state_id: Option<ObjectId>,
        block: GroupConsensusBlock,
    ) {
        let cmd = GroupCommandCommited {
            prev_state_id,
            block,
        };

        let cmd = GroupCommand::from(cmd);
        let object_raw_buf = cmd
            .to_vec()
            .expect(format!("on_commited {} failed for encode", self.non_driver.dec_id()).as_str());
        let any_obj = cyfs_base::AnyNamedObject::Core(
            TypelessCoreObject::clone_from_slice(object_raw_buf.as_slice()).expect(
                format!(
                    "on_commited {} failed for convert to any",
                    self.non_driver.dec_id()
                )
                .as_str(),
            ),
        );

        let result = self
            .non_driver
            .post_object(
                NONObjectInfo {
                    object_id: cmd.desc().object_id(),
                    object_raw: object_raw_buf,
                    object: Some(Arc::new(any_obj)),
                },
                None,
            )
            .await
            .map_err(|err| log::warn!("on_commited {} failed {:?}", self.non_driver.dec_id(), err));

        assert!(result.is_err() || result.unwrap().is_none());
    }
}

#[async_trait::async_trait]
pub trait GroupObjectMapProcessor: Send + Sync {
    async fn create_single_op_env(&self) -> BuckyResult<ObjectMapSingleOpEnvRef>;
    async fn create_sub_tree_op_env(&self) -> BuckyResult<ObjectMapIsolatePathOpEnvRef>;
}

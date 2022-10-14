use super::super::acl::*;
use super::super::local::GlobalStateLocalService;
use super::cache_access::GlobalStateAccessCacheProcessor;
use crate::acl::AclManagerRef;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::ndn::NDNInputProcessorRef;
use crate::non::NONInputProcessorRef;
use crate::root_state::*;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub struct GlobalStateRouter {
    category: GlobalStateCategory,

    global_state_processor: GlobalStateInputProcessorRef,
    op_env_processor: OpEnvInputProcessorRef,
    access_processor: GlobalStateAccessInputProcessorRef,

    zone_manager: ZoneManagerRef,

    forward: ForwardProcessorManager,

    fail_handler: ObjectFailHandler,

    noc_processor: NONInputProcessorRef,
    ndn_processor: NDNInputProcessorRef,
}

impl GlobalStateRouter {
    pub(crate) fn new(
        category: GlobalStateCategory,
        acl: AclManagerRef,
        local_service: GlobalStateLocalService,
        zone_manager: ZoneManagerRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
        noc_processor: NONInputProcessorRef,
        ndn_processor: NDNInputProcessorRef,
    ) -> Self {
        // origin processors
        let global_state_processor = local_service.clone_global_state_processor();
        let op_env_processor = local_service.clone_op_env_processor();

        let access_processor = local_service.clone_access_processor();

        // acl limit processors
        let global_state_processor =
            GlobalStateAclZoneInputProcessor::new(acl.clone(), global_state_processor);
        let op_env_processor = OpEnvAclInnerInputProcessor::new(acl.clone(), op_env_processor);

        let access_processor = GlobalStateAccessAclInputProcessor::new(acl, access_processor);

        Self {
            category,
            global_state_processor,
            op_env_processor,
            access_processor,

            zone_manager,
            forward,
            fail_handler,

            noc_processor,
            ndn_processor,
        }
    }

    pub fn clone_global_state_processor(&self) -> GlobalStateInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    pub fn clone_op_env_processor(&self) -> OpEnvInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    pub fn clone_access_processor(&self) -> GlobalStateAccessInputProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    async fn get_global_state_forward(
        &self,
        target: DeviceId,
    ) -> BuckyResult<GlobalStateInputProcessorRef> {
        // 获取到目标的processor
        let requestor = self.forward.get(&target).await?;

        let processor =
            GlobalStateRequestor::new(self.category.clone(), None, requestor).into_processor();

        // 转换为input processor
        let input_processor = GlobalStateInputTransformer::new(processor);

        Ok(input_processor)
    }

    async fn get_op_env_forward(
        &self,
        sid: u64,
        target: DeviceId,
    ) -> BuckyResult<OpEnvInputProcessorRef> {
        // 获取到目标的processor
        let requestor = self.forward.get(&target).await?;

        let op_env_type = OpEnvSessionIDHelper::get_type(sid)?;

        let processor =
            OpEnvRequestor::new(self.category.clone(), op_env_type, sid, None, requestor)
                .into_processor();

        // 转换为input processor
        let input_processor = OpEnvInputTransformer::new(processor);

        Ok(input_processor)
    }

    async fn get_global_state_access_forward(
        &self,
        target: DeviceId,
    ) -> BuckyResult<GlobalStateAccessInputProcessorRef> {
        // 获取到目标的processor
        let requestor = self.forward.get(&target).await?;

        let processor = GlobalStateAccessRequestor::new(self.category.clone(), None, requestor)
            .into_processor();

        // 转换为input processor
        let input_processor = GlobalStateAccessInputTransformer::new(processor);

        Ok(input_processor)
    }

    // 不同于non/ndn的router，如果target为空，那么表示本地device
    async fn get_target(&self, target: Option<&ObjectId>) -> BuckyResult<Option<DeviceId>> {
        let ret = match target {
            Some(object_id) => {
                let info = self
                    .zone_manager
                    .target_zone_manager()
                    .resolve_target(Some(object_id))
                    .await?;
                if info.target_device == *self.zone_manager.get_current_device_id() {
                    None
                } else {
                    Some(info.target_device)
                }
            }
            None => None,
        };

        Ok(ret)
    }

    async fn get_global_state_processor(
        &self,
        target: Option<&ObjectId>,
    ) -> BuckyResult<GlobalStateInputProcessorRef> {
        if let Some(device_id) = self.get_target(target).await? {
            debug!(
                "global state target resolved: {:?} -> {}",
                target, device_id
            );
            let processor = self.get_global_state_forward(device_id).await?;
            Ok(processor)
        } else {
            Ok(self.global_state_processor.clone())
        }
    }

    async fn get_op_env_processor(
        &self,
        sid: u64,
        target: Option<&ObjectId>,
    ) -> BuckyResult<OpEnvInputProcessorRef> {
        if let Some(device_id) = self.get_target(target).await? {
            debug!("op env target resolved: {:?} -> {}", target, device_id);
            let processor = self.get_op_env_forward(sid, device_id).await?;
            Ok(processor)
        } else {
            Ok(self.op_env_processor.clone())
        }
    }

    async fn get_global_state_access_processor(
        &self,
        target: Option<&ObjectId>,
    ) -> BuckyResult<GlobalStateAccessInputProcessorRef> {
        if let Some(device_id) = self.get_target(target).await? {
            debug!(
                "global state access target resolved: {:?} -> {}",
                target, device_id
            );
            let processor = self.get_global_state_access_forward(device_id).await?;

            // insert a cache level
            let processor = GlobalStateAccessCacheProcessor::new(
                processor,
                self.noc_processor.clone(),
                self.zone_manager.get_current_device_id().to_owned(),
            );

            Ok(processor)
        } else {
            Ok(self.access_processor.clone())
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateInputProcessor for GlobalStateRouter {
    fn create_op_env_processor(&self) -> OpEnvInputProcessorRef {
        self.clone_op_env_processor()
    }

    fn get_category(&self) -> GlobalStateCategory {
        self.category
    }

    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootInputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootInputResponse> {
        let processor = self
            .get_global_state_processor(req.common.target.as_ref())
            .await?;
        processor.get_current_root(req).await
    }

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvInputRequest,
    ) -> BuckyResult<RootStateCreateOpEnvInputResponse> {
        let processor = self
            .get_global_state_processor(req.common.target.as_ref())
            .await?;
        processor.create_op_env(req).await
    }
}

#[async_trait::async_trait]
impl OpEnvInputProcessor for GlobalStateRouter {
    fn get_category(&self) -> GlobalStateCategory {
        self.category
    }

    // single_op_env methods
    async fn load(&self, req: OpEnvLoadInputRequest) -> BuckyResult<()> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.load(req).await
    }

    async fn load_by_path(&self, req: OpEnvLoadByPathInputRequest) -> BuckyResult<()> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.load_by_path(req).await
    }

    async fn create_new(&self, req: OpEnvCreateNewInputRequest) -> BuckyResult<()> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.create_new(req).await
    }

    // lock
    async fn lock(&self, req: OpEnvLockInputRequest) -> BuckyResult<()> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.lock(req).await
    }

    // get_current_root
    async fn get_current_root(
        &self,
        req: OpEnvGetCurrentRootInputRequest,
    ) -> BuckyResult<OpEnvGetCurrentRootInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.get_current_root(req).await
    }

    // transcation
    async fn commit(&self, req: OpEnvCommitInputRequest) -> BuckyResult<OpEnvCommitInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.commit(req).await
    }

    async fn abort(&self, req: OpEnvAbortInputRequest) -> BuckyResult<()> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.abort(req).await
    }

    // map methods
    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyInputRequest,
    ) -> BuckyResult<OpEnvGetByKeyInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.get_by_key(req).await
    }

    async fn insert_with_key(&self, req: OpEnvInsertWithKeyInputRequest) -> BuckyResult<()> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.insert_with_key(req).await
    }

    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyInputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.set_with_key(req).await
    }

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyInputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.remove_with_key(req).await
    }

    // set methods
    async fn contains(
        &self,
        req: OpEnvContainsInputRequest,
    ) -> BuckyResult<OpEnvContainsInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.contains(req).await
    }

    async fn insert(&self, req: OpEnvInsertInputRequest) -> BuckyResult<OpEnvInsertInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.insert(req).await
    }

    async fn remove(&self, req: OpEnvRemoveInputRequest) -> BuckyResult<OpEnvRemoveInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.remove(req).await
    }

    // iterator methods
    async fn next(&self, req: OpEnvNextInputRequest) -> BuckyResult<OpEnvNextInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.next(req).await
    }

    async fn reset(&self, req: OpEnvResetInputRequest) -> BuckyResult<()> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.reset(req).await
    }

    async fn list(&self, req: OpEnvListInputRequest) -> BuckyResult<OpEnvListInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.list(req).await
    }

    // metadata
    async fn metadata(
        &self,
        req: OpEnvMetadataInputRequest,
    ) -> BuckyResult<OpEnvMetadataInputResponse> {
        let processor = self
            .get_op_env_processor(req.common.sid, req.common.target.as_ref())
            .await?;

        processor.metadata(req).await
    }
}

#[async_trait::async_trait]
impl GlobalStateAccessInputProcessor for GlobalStateRouter {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathInputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathInputResponse> {
        let processor = self
            .get_global_state_access_processor(req.common.target.as_ref())
            .await?;
        processor.get_object_by_path(req).await
    }

    async fn list(
        &self,
        req: RootStateAccessListInputRequest,
    ) -> BuckyResult<RootStateAccessListInputResponse> {
        let processor = self
            .get_global_state_access_processor(req.common.target.as_ref())
            .await?;
        processor.list(req).await
    }
}

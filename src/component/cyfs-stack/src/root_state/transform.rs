use super::processor::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 实现从output到input的转换
pub(crate) struct GlobalStateOutputTransformer {
    processor: GlobalStateInputProcessorRef,
    source: RequestSourceInfo,
}

impl GlobalStateOutputTransformer {
    pub fn new(
        processor: GlobalStateInputProcessorRef,
        source: RequestSourceInfo,
    ) -> GlobalStateOutputProcessorRef {
        let ret = Self { processor, source };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: RootStateOutputRequestCommon) -> RootStateInputRequestCommon {
        let mut source = self.source.clone();
        source.set_dec(common.dec_id);

        RootStateInputRequestCommon {
            target_dec_id: common.target_dec_id,

            target: common.target,
            flags: common.flags,

            source,
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateOutputProcessor for GlobalStateOutputTransformer {
    fn get_category(&self) -> GlobalStateCategory {
        self.processor.get_category()
    }

    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootOutputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootOutputResponse> {
        let in_req = RootStateGetCurrentRootInputRequest {
            common: self.convert_common(req.common),
            root_type: req.root_type,
        };

        self.processor.get_current_root(in_req).await
    }

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvOutputRequest,
    ) -> BuckyResult<OpEnvOutputProcessorRef> {
        let in_req = RootStateCreateOpEnvInputRequest {
            common: self.convert_common(req.common),

            op_env_type: req.op_env_type,
        };

        let resp = self.processor.create_op_env(in_req).await?;
        let processor = self.processor.create_op_env_processor();

        let ret = OpEnvOutputTransformer::new(resp.sid, processor, self.source.clone());
        Ok(ret)
    }
}

// 实现从output到input的转换
pub(crate) struct OpEnvOutputTransformer {
    sid: u64,
    processor: OpEnvInputProcessorRef,
    source: RequestSourceInfo,
}

impl OpEnvOutputTransformer {
    pub fn new(
        sid: u64,
        processor: OpEnvInputProcessorRef,
        source: RequestSourceInfo,
    ) -> OpEnvOutputProcessorRef {
        let ret = Self {
            sid,
            processor,
            source,
        };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: OpEnvOutputRequestCommon) -> OpEnvInputRequestCommon {
        let mut source = self.source.clone();
        source.set_dec(common.dec_id);

        OpEnvInputRequestCommon {
            target: common.target,
            flags: common.flags,
            target_dec_id: common.target_dec_id,
            source,
            sid: self.sid,
        }
    }
}

#[async_trait::async_trait]
impl OpEnvOutputProcessor for OpEnvOutputTransformer {
    fn get_sid(&self) -> u64 {
        self.sid
    }

    fn get_category(&self) -> GlobalStateCategory {
        self.processor.get_category()
    }

    async fn load(&self, req: OpEnvLoadOutputRequest) -> BuckyResult<()> {
        let in_req = OpEnvLoadInputRequest {
            common: self.convert_common(req.common),

            target: req.target,
        };

        self.processor.load(in_req).await
    }

    async fn load_by_path(&self, req: OpEnvLoadByPathOutputRequest) -> BuckyResult<()> {
        let in_req = OpEnvLoadByPathInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
        };

        self.processor.load_by_path(in_req).await
    }

    async fn create_new(&self, req: OpEnvCreateNewOutputRequest) -> BuckyResult<()> {
        let in_req = OpEnvCreateNewInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
            content_type: req.content_type,
        };

        self.processor.create_new(in_req).await
    }

    async fn get_current_root(&self, req: OpEnvGetCurrentRootOutputRequest) -> BuckyResult<OpEnvGetCurrentRootOutputResponse> {
        let in_req = OpEnvGetCurrentRootInputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.get_current_root(in_req).await
    }

    async fn lock(&self, req: OpEnvLockOutputRequest) -> BuckyResult<()> {
        let in_req = OpEnvLockInputRequest {
            common: self.convert_common(req.common),

            path_list: req.path_list,
            duration_in_millsecs: req.duration_in_millsecs,
            try_lock: req.try_lock,
        };

        self.processor.lock(in_req).await
    }

    async fn commit(
        &self,
        req: OpEnvCommitOutputRequest,
    ) -> BuckyResult<OpEnvCommitOutputResponse> {
        let in_req = OpEnvCommitInputRequest {
            common: self.convert_common(req.common),
            op_type: req.op_type,
        };

        self.processor.commit(in_req).await
    }

    async fn abort(&self, req: OpEnvAbortOutputRequest) -> BuckyResult<()> {
        let in_req = OpEnvAbortInputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.abort(in_req).await
    }

    // map methods
    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyOutputRequest,
    ) -> BuckyResult<OpEnvGetByKeyOutputResponse> {
        let in_req = OpEnvGetByKeyInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
        };

        self.processor.get_by_key(in_req).await
    }

    async fn insert_with_key(&self, req: OpEnvInsertWithKeyOutputRequest) -> BuckyResult<()> {
        let in_req = OpEnvInsertWithKeyInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
            value: req.value,
        };

        self.processor.insert_with_key(in_req).await
    }

    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyOutputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyOutputResponse> {
        let in_req = OpEnvSetWithKeyInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
            value: req.value,
            prev_value: req.prev_value,
            auto_insert: req.auto_insert,
        };

        self.processor.set_with_key(in_req).await
    }

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyOutputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyOutputResponse> {
        let in_req = OpEnvRemoveWithKeyInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
            prev_value: req.prev_value,
        };

        self.processor.remove_with_key(in_req).await
    }

    // set methods
    async fn contains(
        &self,
        req: OpEnvContainsOutputRequest,
    ) -> BuckyResult<OpEnvContainsOutputResponse> {
        let in_req = OpEnvContainsInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            value: req.value,
        };

        self.processor.contains(in_req).await
    }

    async fn insert(
        &self,
        req: OpEnvInsertOutputRequest,
    ) -> BuckyResult<OpEnvInsertOutputResponse> {
        let in_req = OpEnvInsertInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            value: req.value,
        };

        self.processor.insert(in_req).await
    }

    async fn remove(
        &self,
        req: OpEnvRemoveOutputRequest,
    ) -> BuckyResult<OpEnvRemoveOutputResponse> {
        let in_req = OpEnvRemoveInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            value: req.value,
        };

        self.processor.remove(in_req).await
    }

    // iterator methods
    async fn next(&self, req: OpEnvNextOutputRequest) -> BuckyResult<OpEnvNextOutputResponse> {
        let in_req = OpEnvNextInputRequest {
            common: self.convert_common(req.common),

            step: req.step,
        };

        self.processor.next(in_req).await
    }

    async fn reset(&self, req: OpEnvResetOutputRequest) -> BuckyResult<()> {
        let in_req = OpEnvResetInputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.reset(in_req).await
    }

    async fn list(&self, req: OpEnvListOutputRequest) -> BuckyResult<OpEnvListOutputResponse> {
        let in_req = OpEnvListInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
        };

        self.processor.list(in_req).await
    }

    async fn metadata(
        &self,
        req: OpEnvMetadataOutputRequest,
    ) -> BuckyResult<OpEnvMetadataOutputResponse> {
        let in_req = OpEnvMetadataInputRequest {
            common: self.convert_common(req.common),

            path: req.path,
        };

        self.processor.metadata(in_req).await
    }
}

///////////////////////////////////////////////////

// 实现从input到output的转换
pub(crate) struct GlobalStateInputTransformer {
    processor: GlobalStateOutputProcessorRef,
}

impl GlobalStateInputTransformer {
    pub fn new(processor: GlobalStateOutputProcessorRef) -> GlobalStateInputProcessorRef {
        let ret = Self { processor };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: RootStateInputRequestCommon) -> RootStateOutputRequestCommon {
        RootStateOutputRequestCommon {
            // 来源DEC
            dec_id: common.source.get_opt_dec().cloned(),
            target_dec_id: common.target_dec_id,
            target: common.target,
            flags: common.flags,
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateInputProcessor for GlobalStateInputTransformer {
    fn create_op_env_processor(&self) -> OpEnvInputProcessorRef {
        unreachable!();
    }

    fn get_category(&self) -> GlobalStateCategory {
        self.processor.get_category()
    }

    async fn get_current_root(
        &self,
        req: RootStateGetCurrentRootInputRequest,
    ) -> BuckyResult<RootStateGetCurrentRootInputResponse> {
        let in_req = RootStateGetCurrentRootOutputRequest {
            common: self.convert_common(req.common),
            root_type: req.root_type,
        };

        self.processor.get_current_root(in_req).await
    }

    async fn create_op_env(
        &self,
        req: RootStateCreateOpEnvInputRequest,
    ) -> BuckyResult<RootStateCreateOpEnvInputResponse> {
        let in_req = RootStateCreateOpEnvOutputRequest {
            common: self.convert_common(req.common),

            op_env_type: req.op_env_type,
        };

        let processor = self.processor.create_op_env(in_req).await?;
        let resp = RootStateCreateOpEnvOutputResponse {
            sid: processor.get_sid(),
        };

        Ok(resp)
    }
}

// 实现从output到input的转换
pub(crate) struct OpEnvInputTransformer {
    processor: OpEnvOutputProcessorRef,
}

impl OpEnvInputTransformer {
    pub fn new(processor: OpEnvOutputProcessorRef) -> OpEnvInputProcessorRef {
        let ret = Self { processor };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: OpEnvInputRequestCommon) -> OpEnvOutputRequestCommon {
        OpEnvOutputRequestCommon {
            // 来源DEC
            dec_id: common.source.get_opt_dec().cloned(),
            target_dec_id: common.target_dec_id,
            target: common.target,

            flags: common.flags,

            sid: common.sid,
        }
    }
}

#[async_trait::async_trait]
impl OpEnvInputProcessor for OpEnvInputTransformer {
    fn get_category(&self) -> GlobalStateCategory {
        self.processor.get_category()
    }

    async fn load(&self, req: OpEnvLoadInputRequest) -> BuckyResult<()> {
        let in_req = OpEnvLoadOutputRequest {
            common: self.convert_common(req.common),

            target: req.target,
        };

        self.processor.load(in_req).await
    }

    async fn load_by_path(&self, req: OpEnvLoadByPathInputRequest) -> BuckyResult<()> {
        let in_req = OpEnvLoadByPathOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
        };

        self.processor.load_by_path(in_req).await
    }

    async fn create_new(&self, req: OpEnvCreateNewInputRequest) -> BuckyResult<()> {
        let in_req = OpEnvCreateNewOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
            content_type: req.content_type,
        };

        self.processor.create_new(in_req).await
    }

    async fn get_current_root(&self, req: OpEnvGetCurrentRootInputRequest) -> BuckyResult<OpEnvGetCurrentRootInputResponse> {
        let in_req = OpEnvGetCurrentRootOutputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.get_current_root(in_req).await
    }

    async fn lock(&self, req: OpEnvLockInputRequest) -> BuckyResult<()> {
        let in_req = OpEnvLockOutputRequest {
            common: self.convert_common(req.common),

            path_list: req.path_list,
            duration_in_millsecs: req.duration_in_millsecs,
            try_lock: req.try_lock,
        };

        self.processor.lock(in_req).await
    }

    async fn commit(&self, req: OpEnvCommitInputRequest) -> BuckyResult<OpEnvCommitInputResponse> {
        let in_req = OpEnvCommitOutputRequest {
            common: self.convert_common(req.common),
            op_type: req.op_type,
        };

        self.processor.commit(in_req).await
    }

    async fn abort(&self, req: OpEnvAbortInputRequest) -> BuckyResult<()> {
        let in_req = OpEnvAbortOutputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.abort(in_req).await
    }

    // map methods
    async fn get_by_key(
        &self,
        req: OpEnvGetByKeyInputRequest,
    ) -> BuckyResult<OpEnvGetByKeyInputResponse> {
        let in_req = OpEnvGetByKeyOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
        };

        self.processor.get_by_key(in_req).await
    }

    async fn insert_with_key(&self, req: OpEnvInsertWithKeyInputRequest) -> BuckyResult<()> {
        let in_req = OpEnvInsertWithKeyOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
            value: req.value,
        };

        self.processor.insert_with_key(in_req).await
    }

    async fn set_with_key(
        &self,
        req: OpEnvSetWithKeyInputRequest,
    ) -> BuckyResult<OpEnvSetWithKeyInputResponse> {
        let in_req = OpEnvSetWithKeyOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
            value: req.value,
            prev_value: req.prev_value,
            auto_insert: req.auto_insert,
        };

        self.processor.set_with_key(in_req).await
    }

    async fn remove_with_key(
        &self,
        req: OpEnvRemoveWithKeyInputRequest,
    ) -> BuckyResult<OpEnvRemoveWithKeyInputResponse> {
        let in_req = OpEnvRemoveWithKeyOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            key: req.key,
            prev_value: req.prev_value,
        };

        self.processor.remove_with_key(in_req).await
    }

    // set methods
    async fn contains(
        &self,
        req: OpEnvContainsInputRequest,
    ) -> BuckyResult<OpEnvContainsInputResponse> {
        let in_req = OpEnvContainsOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            value: req.value,
        };

        self.processor.contains(in_req).await
    }

    async fn insert(&self, req: OpEnvInsertInputRequest) -> BuckyResult<OpEnvInsertInputResponse> {
        let in_req = OpEnvInsertOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            value: req.value,
        };

        self.processor.insert(in_req).await
    }

    async fn remove(&self, req: OpEnvRemoveInputRequest) -> BuckyResult<OpEnvRemoveInputResponse> {
        let in_req = OpEnvRemoveOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
            value: req.value,
        };

        self.processor.remove(in_req).await
    }

    // iterator methods
    async fn next(&self, req: OpEnvNextInputRequest) -> BuckyResult<OpEnvNextInputResponse> {
        let in_req = OpEnvNextOutputRequest {
            common: self.convert_common(req.common),

            step: req.step,
        };

        self.processor.next(in_req).await
    }

    async fn reset(&self, req: OpEnvResetInputRequest) -> BuckyResult<()> {
        let in_req = OpEnvResetOutputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.reset(in_req).await
    }

    async fn list(&self, req: OpEnvListInputRequest) -> BuckyResult<OpEnvListInputResponse> {
        let in_req = OpEnvListOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
        };

        self.processor.list(in_req).await
    }

    async fn metadata(
        &self,
        req: OpEnvMetadataInputRequest,
    ) -> BuckyResult<OpEnvMetadataInputResponse> {
        let in_req = OpEnvMetadataOutputRequest {
            common: self.convert_common(req.common),

            path: req.path,
        };

        self.processor.metadata(in_req).await
    }
}

// 实现从output到input的转换
pub(crate) struct GlobalStateAccessOutputTransformer {
    processor: GlobalStateAccessInputProcessorRef,
    source: RequestSourceInfo,
}

impl GlobalStateAccessOutputTransformer {
    pub fn new(
        processor: GlobalStateAccessInputProcessorRef,
        source: RequestSourceInfo,
    ) -> GlobalStateAccessOutputProcessorRef {
        let ret = Self { processor, source };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: RootStateOutputRequestCommon) -> RootStateInputRequestCommon {
        let mut source = self.source.clone();
        source.set_dec(common.dec_id);

        RootStateInputRequestCommon {
            // 来源DEC
            target_dec_id: common.target_dec_id,
            target: common.target,
            flags: common.flags,

            source,
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateAccessOutputProcessor for GlobalStateAccessOutputTransformer {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathOutputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathOutputResponse> {
        let in_req = RootStateAccessGetObjectByPathInputRequest {
            common: self.convert_common(req.common),
            inner_path: req.inner_path,
        };

        let in_resp = self.processor.get_object_by_path(in_req).await?;

        let resp = RootStateAccessGetObjectByPathOutputResponse {
            object: NONGetObjectOutputResponse {
                object: in_resp.object.object,
                object_expires_time: in_resp.object.object_expires_time,
                object_update_time: in_resp.object.object_update_time,
                attr: in_resp.object.attr,
            },
            root: in_resp.root,
            revision: in_resp.revision,
        };

        Ok(resp)
    }

    async fn list(
        &self,
        req: RootStateAccessListOutputRequest,
    ) -> BuckyResult<RootStateAccessListOutputResponse> {
        let in_req = RootStateAccessListInputRequest {
            common: self.convert_common(req.common),
            page_index: req.page_index,
            page_size: req.page_size,
            inner_path: req.inner_path,
        };

        self.processor.list(in_req).await
    }
}

// 实现从input到output的转换
pub(crate) struct GlobalStateAccessInputTransformer {
    processor: GlobalStateAccessOutputProcessorRef,
}

impl GlobalStateAccessInputTransformer {
    pub fn new(
        processor: GlobalStateAccessOutputProcessorRef,
    ) -> GlobalStateAccessInputProcessorRef {
        let ret = Self { processor };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: RootStateInputRequestCommon) -> RootStateOutputRequestCommon {
        RootStateOutputRequestCommon {
            // 来源DEC
            dec_id: common.source.get_opt_dec().cloned(),
            target_dec_id: common.target_dec_id,
            target: common.target,
            flags: common.flags,
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateAccessInputProcessor for GlobalStateAccessInputTransformer {
    async fn get_object_by_path(
        &self,
        req: RootStateAccessGetObjectByPathInputRequest,
    ) -> BuckyResult<RootStateAccessGetObjectByPathInputResponse> {
        let out_req = RootStateAccessGetObjectByPathOutputRequest {
            common: self.convert_common(req.common),
            inner_path: req.inner_path,
        };

        let out_resp = self.processor.get_object_by_path(out_req).await?;

        let resp = RootStateAccessGetObjectByPathInputResponse {
            object: NONGetObjectInputResponse {
                object: out_resp.object.object,
                object_expires_time: out_resp.object.object_expires_time,
                object_update_time: out_resp.object.object_update_time,
                attr: out_resp.object.attr,
            },
            root: out_resp.root,
            revision: out_resp.revision,
        };

        Ok(resp)
    }

    async fn list(
        &self,
        req: RootStateAccessListInputRequest,
    ) -> BuckyResult<RootStateAccessListInputResponse> {
        let out_req = RootStateAccessListOutputRequest {
            common: self.convert_common(req.common),
            page_index: req.page_index,
            page_size: req.page_size,
            inner_path: req.inner_path,
        };

        self.processor.list(out_req).await
    }
}

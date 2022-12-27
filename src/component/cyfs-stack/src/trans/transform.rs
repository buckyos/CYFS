use crate::trans::{TransInputProcessor, TransInputProcessorRef};
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct TransInputTransformer {
    processor: TransOutputProcessorRef,
}

impl TransInputTransformer {
    pub fn new(processor: TransOutputProcessorRef) -> TransInputProcessorRef {
        Arc::new(Box::new(Self { processor }))
    }

    fn convert_common(common: NDNInputRequestCommon) -> NDNOutputRequestCommon {
        NDNOutputRequestCommon {
            req_path: common.req_path,
            dec_id: Some(common.source.dec),
            level: common.level,
            target: common.target,
            referer_object: common.referer_object,
            flags: common.flags,
        }
    }
}

#[async_trait::async_trait]
impl TransInputProcessor for TransInputTransformer {
    async fn get_context(&self, req: TransGetContextInputRequest) -> BuckyResult<TransGetContextInputResponse> {
        let out_req = TransGetContextOutputRequest {
            common: Self::convert_common(req.common),
            context_id: req.context_id,
            context_path: req.context_path,
        };
        let out_resp = self.processor.get_context(out_req).await?;
        Ok(out_resp)
    }

    async fn put_context(&self, req: TransUpdateContextInputRequest) -> BuckyResult<()> {
        let out_req = TransPutContextOutputRequest {
            common: Self::convert_common(req.common),
            context: req.context,
            access: req.access,
        };
        let out_resp = self.processor.put_context(out_req).await?;
        Ok(out_resp)
    }

    async fn create_task(
        &self,
        req: TransCreateTaskInputRequest,
    ) -> BuckyResult<TransCreateTaskInputResponse> {
        let out_req = TransCreateTaskOutputRequest {
            common: Self::convert_common(req.common),
            object_id: req.object_id,
            local_path: req.local_path,
            device_list: req.device_list,
            group: req.group,
            context: req.context,
            auto_start: req.auto_start,
        };

        let out_resp = self.processor.create_task(out_req).await?;
        Ok(TransCreateTaskInputResponse {
            task_id: out_resp.task_id,
        })
    }

    async fn control_task(&self, req: TransControlTaskInputRequest) -> BuckyResult<()> {
        self.processor
            .control_task(TransControlTaskOutputRequest {
                common: Self::convert_common(req.common),
                task_id: req.task_id,
                action: req.action,
            })
            .await
    }

    async fn query_tasks(
        &self,
        req: TransQueryTasksInputRequest,
    ) -> BuckyResult<TransQueryTasksInputResponse> {
        let out_req = TransQueryTasksOutputRequest {
            common: Self::convert_common(req.common),
            task_status: req.task_status,
            range: req.range,
        };
        let out_resp = self.processor.query_tasks(out_req).await?;
        Ok(TransQueryTasksInputResponse {
            task_list: out_resp.task_list,
        })
    }

    async fn get_task_state(
        &self,
        req: TransGetTaskStateInputRequest,
    ) -> BuckyResult<TransGetTaskStateInputResponse> {
        let out_req = TransGetTaskStateOutputRequest {
            common: Self::convert_common(req.common),
            task_id: req.task_id.clone(),
        };
        let out_resp = self.processor.get_task_state(out_req).await?;
        Ok(out_resp)
    }

    async fn publish_file(
        &self,
        req: TransPublishFileInputRequest,
    ) -> BuckyResult<TransPublishFileInputResponse> {
        let out_req = TransPublishFileOutputRequest {
            common: Self::convert_common(req.common),
            owner: req.owner,
            local_path: req.local_path,
            chunk_size: req.chunk_size,
            file_id: req.file_id,
            dirs: req.dirs,
        };

        let out_resp = self.processor.publish_file(out_req).await?;

        Ok(TransPublishFileInputResponse {
            file_id: out_resp.file_id,
        })
    }

    async fn get_task_group_state(
        &self,
        req: TransGetTaskGroupStateInputRequest,
    ) -> BuckyResult<TransGetTaskGroupStateInputResponse> {
        let out_req = TransGetTaskGroupStateOutputRequest {
            common: Self::convert_common(req.common),
            group: req.group.clone(),
            speed_when: req.speed_when,
        };

        self.processor.get_task_group_state(out_req).await
    }

    async fn control_task_group(
        &self,
        req: TransControlTaskGroupInputRequest,
    ) -> BuckyResult<TransControlTaskGroupInputResponse> {
        let out_req = TransControlTaskGroupOutputRequest {
            common: Self::convert_common(req.common),
            group: req.group.clone(),
            action: req.action.clone(),
        };

        self.processor.control_task_group(out_req).await
    }
}

pub(crate) struct TransOutputTransformer {
    processor: TransInputProcessorRef,
    source: RequestSourceInfo,
}

impl TransOutputTransformer {
    pub(crate) fn new(
        processor: TransInputProcessorRef,
        source: RequestSourceInfo,
    ) -> TransOutputProcessorRef {
        Arc::new(Self { processor, source })
    }

    fn convert_common(&self, common: NDNOutputRequestCommon) -> NDNInputRequestCommon {
        let mut source = self.source.clone();
        if let Some(dec_id) = common.dec_id {
            source.set_dec(dec_id);
        }

        NDNInputRequestCommon {
            req_path: common.req_path,
            source,
            level: common.level,
            referer_object: common.referer_object,
            target: common.target,
            flags: common.flags,
            user_data: None,
        }
    }
}

#[async_trait::async_trait]
impl TransOutputProcessor for TransOutputTransformer {
    async fn get_context(&self, req: TransGetContextOutputRequest) -> BuckyResult<TransGetContextOutputResponse> {
        let in_req = TransGetContextInputRequest {
            common: self.convert_common(req.common),
            context_id: req.context_id,
            context_path: req.context_path,
        };

        let in_resp = self.processor.get_context(in_req).await?;
        Ok(in_resp)
    }

    async fn put_context(&self, req: TransPutContextOutputRequest) -> BuckyResult<()> {
        let in_req = TransUpdateContextInputRequest {
            common: self.convert_common(req.common),
            context: req.context,
            access: req.access,
        };

        let in_resp = self.processor.put_context(in_req).await?;
        Ok(in_resp)
    }

    async fn get_task_state(
        &self,
        req: TransGetTaskStateOutputRequest,
    ) -> BuckyResult<TransGetTaskStateOutputResponse> {
        let in_req = TransGetTaskStateInputRequest {
            common: self.convert_common(req.common),
            task_id: req.task_id,
        };

        let in_resp = self.processor.get_task_state(in_req).await?;
        Ok(in_resp)
    }

    async fn publish_file(
        &self,
        req: TransPublishFileOutputRequest,
    ) -> BuckyResult<TransPublishFileOutputResponse> {
        let in_req = TransPublishFileInputRequest {
            common: self.convert_common(req.common),
            owner: req.owner,
            local_path: req.local_path,
            chunk_size: req.chunk_size,
            file_id: req.file_id,
            dirs: req.dirs,
        };

        let in_resp = self.processor.publish_file(in_req).await?;

        Ok(TransPublishFileOutputResponse {
            file_id: in_resp.file_id,
        })
    }

    async fn create_task(
        &self,
        req: TransCreateTaskOutputRequest,
    ) -> BuckyResult<TransCreateTaskOutputResponse> {
        let in_req = TransCreateTaskInputRequest {
            common: self.convert_common(req.common),
            object_id: req.object_id,
            local_path: req.local_path,
            device_list: req.device_list,
            group: req.group,
            context: req.context,
            auto_start: req.auto_start,
        };

        let in_resp = self.processor.create_task(in_req).await?;
        Ok(TransCreateTaskOutputResponse {
            task_id: in_resp.task_id,
        })
    }

    async fn query_tasks(
        &self,
        req: TransQueryTasksOutputRequest,
    ) -> BuckyResult<TransQueryTasksOutputResponse> {
        let in_req = TransQueryTasksInputRequest {
            common: self.convert_common(req.common),
            task_status: req.task_status,
            range: req.range,
        };
        let in_resp = self.processor.query_tasks(in_req).await?;
        Ok(TransQueryTasksOutputResponse {
            task_list: in_resp.task_list,
        })
    }

    async fn control_task(&self, req: TransControlTaskOutputRequest) -> BuckyResult<()> {
        self.processor
            .control_task(TransControlTaskInputRequest {
                common: self.convert_common(req.common),
                task_id: req.task_id,
                action: req.action,
            })
            .await
    }

    async fn get_task_group_state(
        &self,
        req: TransGetTaskGroupStateOutputRequest,
    ) -> BuckyResult<TransGetTaskGroupStateOutputResponse> {
        let in_req = TransGetTaskGroupStateInputRequest {
            common: self.convert_common(req.common),
            group: req.group,
            speed_when: req.speed_when,
        };

        self.processor.get_task_group_state(in_req).await
    }

    async fn control_task_group(
        &self,
        req: TransControlTaskGroupOutputRequest,
    ) -> BuckyResult<TransControlTaskGroupOutputResponse> {
        let in_req = TransControlTaskGroupInputRequest {
            common: self.convert_common(req.common),
            group: req.group,
            action: req.action,
        };

        self.processor.control_task_group(in_req).await
    }
}

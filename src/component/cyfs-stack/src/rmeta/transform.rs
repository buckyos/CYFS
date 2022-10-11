use super::processor::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

// 实现从output到input的转换
pub(crate) struct GlobalStateMetaOutputTransformer {
    processor: GlobalStateMetaInputProcessorRef,
    source: RequestSourceInfo,
}

impl GlobalStateMetaOutputTransformer {
    pub fn new(
        processor: GlobalStateMetaInputProcessorRef,
        source: RequestSourceInfo,
    ) -> GlobalStateMetaOutputProcessorRef {
        let ret = Self { processor, source };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: MetaOutputRequestCommon) -> MetaInputRequestCommon {
        let mut source = self.source.clone();
        if let Some(dec_id) = common.dec_id {
            source.set_dec(dec_id);
        }

        MetaInputRequestCommon {
            target: common.target,
            flags: common.flags,
            target_dec_id: common.target_dec_id,
            source,
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateMetaOutputProcessor for GlobalStateMetaOutputTransformer {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessOutputResponse> {
        let in_req = GlobalStateMetaAddAccessInputRequest {
            common: self.convert_common(req.common),
            item: req.item,
        };

        self.processor.add_access(in_req).await
    }

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessOutputResponse> {
        let in_req = GlobalStateMetaRemoveAccessInputRequest {
            common: self.convert_common(req.common),
            item: req.item,
        };

        self.processor.remove_access(in_req).await
    }

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessOutputResponse> {
        let in_req = GlobalStateMetaClearAccessInputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.clear_access(in_req).await
    }

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkOutputResponse> {
        let in_req = GlobalStateMetaAddLinkInputRequest {
            common: self.convert_common(req.common),
            source: req.source,
            target: req.target,
        };

        self.processor.add_link(in_req).await
    }

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkOutputResponse> {
        let in_req = GlobalStateMetaRemoveLinkInputRequest {
            common: self.convert_common(req.common),
            source: req.source,
        };

        self.processor.remove_link(in_req).await
    }

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkOutputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkOutputResponse> {
        let in_req = GlobalStateMetaClearLinkInputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.clear_link(in_req).await
    }
}

///////////////////////////////////////////////////

// 实现从input到output的转换
pub(crate) struct GlobalStateMetaInputTransformer {
    processor: GlobalStateMetaOutputProcessorRef,
}

impl GlobalStateMetaInputTransformer {
    pub fn new(processor: GlobalStateMetaOutputProcessorRef) -> GlobalStateMetaInputProcessorRef {
        let ret = Self { processor };
        Arc::new(Box::new(ret))
    }

    fn convert_common(&self, common: MetaInputRequestCommon) -> MetaOutputRequestCommon {
        MetaOutputRequestCommon {
            // 来源DEC
            dec_id: Some(common.source.dec),
            target_dec_id: common.target_dec_id,
            target: common.target,
            flags: common.flags,
        }
    }
}

#[async_trait::async_trait]
impl GlobalStateMetaInputProcessor for GlobalStateMetaInputTransformer {
    async fn add_access(
        &self,
        req: GlobalStateMetaAddAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddAccessInputResponse> {
        let in_req = GlobalStateMetaAddAccessOutputRequest {
            common: self.convert_common(req.common),
            item: req.item,
        };

        self.processor.add_access(in_req).await
    }

    async fn remove_access(
        &self,
        req: GlobalStateMetaRemoveAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveAccessInputResponse> {
        let in_req = GlobalStateMetaRemoveAccessOutputRequest {
            common: self.convert_common(req.common),
            item: req.item,
        };

        self.processor.remove_access(in_req).await
    }

    async fn clear_access(
        &self,
        req: GlobalStateMetaClearAccessInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearAccessInputResponse> {
        let in_req = GlobalStateMetaClearAccessOutputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.clear_access(in_req).await
    }

    async fn add_link(
        &self,
        req: GlobalStateMetaAddLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaAddLinkInputResponse> {
        let in_req = GlobalStateMetaAddLinkOutputRequest {
            common: self.convert_common(req.common),
            source: req.source,
            target: req.target,
        };

        self.processor.add_link(in_req).await
    }

    async fn remove_link(
        &self,
        req: GlobalStateMetaRemoveLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaRemoveLinkInputResponse> {
        let in_req = GlobalStateMetaRemoveLinkOutputRequest {
            common: self.convert_common(req.common),
            source: req.source,
        };

        self.processor.remove_link(in_req).await
    }

    async fn clear_link(
        &self,
        req: GlobalStateMetaClearLinkInputRequest,
    ) -> BuckyResult<GlobalStateMetaClearLinkInputResponse> {
        let in_req = GlobalStateMetaClearLinkOutputRequest {
            common: self.convert_common(req.common),
        };

        self.processor.clear_link(in_req).await
    }
}

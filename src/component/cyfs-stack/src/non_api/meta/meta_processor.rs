use super::super::file::NONFileServiceProcessor;
use crate::meta::*;
use crate::ndn_api::NDCLevelInputProcessor;
use crate::non::*;
use crate::resolver::OodResolver;
use cyfs_base::*;
use cyfs_lib::*;


use std::sync::Arc;
use cyfs_chunk_cache::ChunkManager;

pub(crate) struct MetaInputProcessor {
    next: Option<NONInputProcessorRef>,
    meta_cache: Box<dyn MetaCache>,
}

impl MetaInputProcessor {
    fn new_raw(next: Option<NONInputProcessorRef>, meta_cache: Box<dyn MetaCache>) -> NONInputProcessorRef {
        let ret = Self { next, meta_cache };
        Arc::new(Box::new(ret))
    }

    // 带file服务的noc processor
    pub(crate) fn new_raw_with_file_service(
        noc_processor: Option<NONInputProcessorRef>,
        meta_cache: Box<dyn MetaCache>,
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        ood_resolver: OodResolver,
        chunk_manager: Arc<ChunkManager>,
        noc: Box<dyn NamedObjectCache>,
    ) -> NONInputProcessorRef {
        let meta_processor = Self::new_raw(noc_processor, meta_cache);

        let ndc_processor = NDCLevelInputProcessor::new_raw(chunk_manager, ndc, tracker, meta_processor.clone());

        let file_processor = NONFileServiceProcessor::new(
            NONAPILevel::NOC,
            meta_processor,
            ndc_processor,
            ood_resolver,
            noc,
        );

        file_processor
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        // 如果开启了flush标志位，那么不首先从noc获取
        let flush = (req.common.flags & CYFS_ROUTER_REQUEST_FLAG_FLUSH) != 0;

        if let Some(next) = &self.next {
            if flush {
                if let Ok(resp) = self.get_from_meta(&req).await {
                    return Ok(resp);
                }

                next.get_object(req).await
            } else {
                if let Ok(resp) = next.get_object(req.clone()).await {
                    return Ok(resp);
                }

                self.get_from_meta(&req).await
            }
        } else {
            self.get_from_meta(&req).await
        }
    }

    // 从meta查询，目前只有主ood才会从meta查询(其余device查询会导致冗余操作？)
    async fn get_from_meta(
        &self,
        req: &NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let meta_resp = self.meta_cache.get_object(&req.object_id).await?;
        match meta_resp {
            Some(data) => {
                let mut resp = NONGetObjectInputResponse::new(
                    req.object_id.clone(),
                    data.object_raw,
                    Some(data.object),
                );

                resp.init_times()?;

                Ok(resp)
            }
            None => {
                let msg = format!(
                    "get object from meta chain but not found! {}",
                    req.object_id
                );
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for MetaInputProcessor {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        if let Some(next) = &self.next {
            next.put_object(req).await
        } else {
            unreachable!();
        }
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        Self::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        if let Some(next) = &self.next {
            next.post_object(req).await
        } else {
            unreachable!();
        }
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        if let Some(next) = &self.next {
            next.select_object(req).await
        } else {
            unreachable!();
        }
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        if let Some(next) = &self.next {
            next.delete_object(req).await
        } else {
            unreachable!();
        }
    }
}

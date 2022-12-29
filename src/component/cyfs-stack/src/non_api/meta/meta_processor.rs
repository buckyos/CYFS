use super::super::inner_path::NONInnerPathServiceProcessor;
use crate::NamedDataComponents;
use crate::meta::*;
use crate::non::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

pub(crate) struct MetaInputProcessor {
    next: Option<NONInputProcessorRef>,
    meta_cache: MetaCacheRef,
}

impl MetaInputProcessor {
    fn new(
        next: Option<NONInputProcessorRef>,
        meta_cache: MetaCacheRef,
    ) -> NONInputProcessorRef {
        let ret = Self { next, meta_cache };
        Arc::new(Box::new(ret))
    }

    // Integrate noc with inner_path+meta service
    pub(crate) fn new_with_inner_path_service(
        noc_processor: Option<NONInputProcessorRef>,
        meta_cache: MetaCacheRef,
        named_data_components: &NamedDataComponents,
        noc: NamedObjectCacheRef,
    ) -> NONInputProcessorRef {
        let noc_with_meta_processor = Self::new(noc_processor, meta_cache);

        let inner_path_processor = NONInnerPathServiceProcessor::new(
            noc_with_meta_processor,
            named_data_components,
            noc,
        );

        inner_path_processor
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

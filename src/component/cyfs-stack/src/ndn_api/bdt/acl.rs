use super::super::acl::NDNAclInputProcessor;
use super::super::handler::*;
use super::cache::*;
use super::echo::BdtNdnEchoProcessor;
use crate::acl::AclManagerRef;
use crate::ndn::*;
use crate::ndn_api::LocalDataManager;
use crate::non::NONInputProcessorRef;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::acl::*;

use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct BdtNDNDataAclProcessor {
    zone_manager: ZoneManagerRef,
    processor: Arc<NDNAclInputProcessor>,
    cache: BdtDataAclCache,
}

impl BdtNDNDataAclProcessor {
    pub(crate) fn new(
        zone_manager: ZoneManagerRef,
        acl: AclManagerRef,
        router_handlers: RouterHandlersManager,
        data_manager: LocalDataManager,
    ) -> Self {
        // 最终的反射应答处理器
        let echo = BdtNdnEchoProcessor::new();

        // 添加pre-router的事件处理器
        let handler_processor =
            NDNHandlerPreProcessor::new(RouterHandlerChain::PreRouter, echo, router_handlers);

        // TODO 是否需要post-router的事件处理器?

        // 添加acl
        let processor = NDNAclInputProcessor::new(acl, data_manager, handler_processor);

        let cache = BdtDataAclCache::new();

        Self {
            zone_manager,
            processor: Arc::new(processor),
            cache,
        }
    }

    pub fn bind_non_processor(&self, non_processor: NONInputProcessorRef) {
        self.processor.bind_non_processor(non_processor)
    }

    fn process_resp<T>(resp: BuckyResult<T>) -> BuckyResult<()> {
        match resp {
            Err(e) => {
                debug!("bdt processor acl response: {}", e);
                match e.code() {
                    BuckyErrorCode::NotImplement => Ok(()),
                    _ => Err(e),
                }
            }
            Ok(_) => unreachable!(),
        }
    }

    async fn get_data_without_cache(&self, req: BdtGetDataInputRequest) -> BuckyResult<()> {
        info!("will process bdt get_data acl request: {}", req);

        let referer = if let Some(referer) = req.referer {
            Some(BdtDataRefererInfo::decode_string(&referer)?)
        } else {
            None
        };

        // first resolve the request's source
        let dec = if let Some(referer) = &referer {
            &referer.dec_id
        } else {
            &None
        };

        let source = self
            .zone_manager
            .resolve_source_info(dec, req.source)
            .await?;

        // check if need verify by acl at top level
        let access_without_acl = if let Some(referer) = &referer {
            if referer.req_path.is_none()
                && referer.referer_object.is_empty()
                && referer.object_id.obj_type_code() == ObjectTypeCode::Chunk
            {
                true
            } else {
                false
            }
        } else {
            true
        };

        if access_without_acl {
            if source.is_current_zone() {
                // In the same zone, if you know the chunk_id, you can access it directly
                return Ok(());
            } else {
                let msg = format!(
                    "bdt get_data but neither referer_object nor req_path are specified! id={}",
                    req.object_id
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
            }
        }

        let mut ndn_req = NDNGetDataInputRequest {
            common: NDNInputRequestCommon {
                req_path: None,
                source,
                level: NDNAPILevel::Router,
                referer_object: vec![],
                target: None,
                flags: 0,
                user_data: None,
            },
            object_id: req.object_id,
            data_type: NDNDataType::Mem,

            // FIXME 这里bdt判定权限是否需要range信息？
            range: None,

            inner_path: None,
        };

        if let Some(referer) = referer {
            // bdt回调都是chunk粒度的，所以我们需要在referer里面保存请求对应的file或者dir+inner_path
            ndn_req.object_id = referer.object_id;
            ndn_req.inner_path = referer.inner_path;
            ndn_req.common.req_path = referer.req_path;
            ndn_req.common.flags = referer.flags;
            if referer.referer_object.len() > 0 {
                ndn_req.common.referer_object = referer.referer_object;
            }
        }

        let resp = self.processor.get_data(ndn_req).await;
        Self::process_resp(resp)
    }

    pub async fn get_data(&self, mut req: BdtGetDataInputRequest) -> BuckyResult<()> {
        let key = BdtDataAclCacheKey {
            source: req.source.clone(),
            referer: req.referer,
            action: NDNAction::GetData,
        };

        if let Some(ret) = self.cache.get(&key) {
            info!(
                "bdt get_data acl request hit cache: ret={:?}, object={}",
                ret, req.object_id
            );
            return ret;
        }

        req.referer = key.referer.clone();
        let ret = self.get_data_without_cache(req).await;
        self.cache.add(key, ret.clone());

        ret
    }
}

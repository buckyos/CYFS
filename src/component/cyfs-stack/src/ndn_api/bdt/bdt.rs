use super::super::acl::NDNAclInputProcessor;
use super::super::handler::*;
use super::cache::*;
use super::echo::NDNBdtEchoProcessor;
use crate::ndn::*;
use crate::acl::AclManagerRef;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::acl::*;

#[derive(Clone)]
pub(crate) struct NDNBdtDataAclProcessor {
    zone_manager: ZoneManagerRef,
    processor: NDNInputProcessorRef,
    cache: BdtDataAclCache,
}

impl NDNBdtDataAclProcessor {
    pub fn new(
        zone_manager: ZoneManagerRef,
        acl: AclManagerRef,
        router_handlers: RouterHandlersManager,
    ) -> Self {
        // 最终的反射应答处理器
        let echo = NDNBdtEchoProcessor::new();

        // 添加pre-router的事件处理器
        let handler_processor =
            NDNHandlerPreProcessor::new(RouterHandlerChain::PreRouter, echo, router_handlers);

        // TODO 是否需要post-router的事件处理器?

        // 添加acl
        let processor = NDNAclInputProcessor::new(acl, handler_processor);

        let cache = BdtDataAclCache::new();

        Self {
            zone_manager,
            processor,
            cache,
        }
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

        let dec = if let Some(referer) = &referer {
            &referer.dec_id
        } else {
            &None
        };

        let source = self
            .zone_manager
            .resolve_source_info(dec, req.source)
            .await?;

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

    async fn put_data_without_cache(&self, req: BdtPutDataInputRequest) -> BuckyResult<()> {
        info!("will process bdt put_data acl request: {}", req);

        let referer = if let Some(referer) = req.referer {
            Some(BdtDataRefererInfo::decode_string(&referer)?)
        } else {
            None
        };

        let dec = if let Some(referer) = &referer {
            &referer.dec_id
        } else {
            &None
        };

        let source = self
            .zone_manager
            .resolve_source_info(dec, req.source)
            .await?;

        let mut ndn_req = NDNPutDataInputRequest {
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
            length: req.length,
            data: Box::new(async_std::io::Cursor::new(vec![])),
        };

        if let Some(referer) = referer {
            ndn_req.object_id = referer.object_id;
            ndn_req.common.req_path = referer.req_path;
            ndn_req.common.flags = referer.flags;
            if referer.referer_object.len() > 0 {
                ndn_req.common.referer_object = referer.referer_object;
            }
        }

        let resp = self.processor.put_data(ndn_req).await;
        Self::process_resp(resp)
    }

    async fn delete_data_without_cache(&self, req: BdtDeleteDataInputRequest) -> BuckyResult<()> {
        info!("will process bdt delete_data acl request: {}", req);

        let referer = if let Some(referer) = req.referer {
            Some(BdtDataRefererInfo::decode_string(&referer)?)
        } else {
            None
        };

        let dec = if let Some(referer) = &referer {
            &referer.dec_id
        } else {
            &None
        };

        let source = self
            .zone_manager
            .resolve_source_info(dec, req.source)
            .await?;

        let mut ndn_req = NDNDeleteDataInputRequest {
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
            inner_path: None,
        };

        if let Some(referer) = referer {
            ndn_req.object_id = referer.object_id;
            ndn_req.inner_path = referer.inner_path;
            ndn_req.common.req_path = referer.req_path;
            ndn_req.common.flags = referer.flags;
            if referer.referer_object.len() > 0 {
                ndn_req.common.referer_object = referer.referer_object;
            }
        }

        let resp = self.processor.delete_data(ndn_req).await;
        Self::process_resp(resp)
    }
}

#[async_trait::async_trait]
impl BdtDataAclProcessor for NDNBdtDataAclProcessor {
    async fn get_data(&self, mut req: BdtGetDataInputRequest) -> BuckyResult<()> {
        let key = BdtDataAclCacheKey {
            source: req.source.clone(),
            referer: req.referer,
            action: NDNAction::GetData,
        };

        if let Some(ret) = self.cache.get(&key) {
            info!(
                "bdt get_data acl request got cache: ret={:?}, object={}",
                ret, req.object_id
            );
            return ret;
        }

        req.referer = key.referer.clone();
        let ret = self.get_data_without_cache(req).await;
        self.cache.add(key, ret.clone());

        ret
    }

    async fn put_data(&self, mut req: BdtPutDataInputRequest) -> BuckyResult<()> {
        info!("will process bdt put_data acl request: {}", req);

        let key = BdtDataAclCacheKey {
            source: req.source.clone(),
            referer: req.referer,
            action: NDNAction::PutData,
        };

        if let Some(ret) = self.cache.get(&key) {
            info!(
                "bdt put_data acl request got cache: ret={:?}, object={}",
                ret, req.object_id
            );
            return ret;
        }

        req.referer = key.referer.clone();
        let ret = self.put_data_without_cache(req).await;
        self.cache.add(key, ret.clone());

        ret
    }

    async fn delete_data(&self, mut req: BdtDeleteDataInputRequest) -> BuckyResult<()> {
        info!("will process bdt delete_data acl request: {}", req);

        let key = BdtDataAclCacheKey {
            source: req.source.clone(),
            referer: req.referer,
            action: NDNAction::DeleteData,
        };

        if let Some(ret) = self.cache.get(&key) {
            info!(
                "bdt delete_data acl request got cache: ret={:?}, object={}",
                ret, req.object_id
            );
            return ret;
        }

        req.referer = key.referer.clone();
        let ret = self.delete_data_without_cache(req).await;
        self.cache.add(key, ret.clone());

        ret
    }
}

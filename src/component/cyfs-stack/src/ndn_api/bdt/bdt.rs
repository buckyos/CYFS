use super::super::acl::NDNAclInputProcessor;
use super::super::handler::*;
use super::cache::*;
use super::echo::NDNBdtEchoProcessor;
use crate::acl::*;
use crate::ndn::*;
use crate::router_handler::RouterHandlersManager;
use cyfs_util::acl::*;
use cyfs_base::*;
use cyfs_lib::*;

#[derive(Clone)]
pub(crate) struct NDNBdtDataAclProcessor {
    processor: NDNInputProcessorRef,
    cache: BdtDataAclCache,
}

impl NDNBdtDataAclProcessor {
    pub fn new(acl: AclManagerRef, router_handlers: RouterHandlersManager) -> Self {
        // 最终的反射应答处理器
        let echo = NDNBdtEchoProcessor::new();

        // 添加pre-router的事件处理器
        let handler_processor =
            NDNHandlerPreProcessor::new(RouterHandlerChain::PreRouter, echo, router_handlers);

        // TODO 是否需要post-router的事件处理器?

        // 添加acl
        let processor = NDNAclInputProcessor::new(acl, handler_processor);

        let cache = BdtDataAclCache::new();

        Self { processor, cache }
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

    fn fill_common(
        common: &mut NDNInputRequestCommon,
        referer: BdtDataRefererInfo,
    ) -> BuckyResult<()> {
        common.dec_id = referer.dec_id;
        common.req_path = referer.req_path;
        common.flags = referer.flags;
        if referer.referer_object.len() > 0 {
            common.referer_object = referer.referer_object;
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl BdtDataAclProcessor for NDNBdtDataAclProcessor {
    async fn get_data(&self, req: BdtGetDataInputRequest) -> BuckyResult<()> {
        info!("will process bdt get_data acl request: {}", req);

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

        let mut ndn_req = NDNGetDataInputRequest {
            common: NDNInputRequestCommon {
                req_path: None,
                dec_id: None,
                source: req.source,
                protocol: NONProtocol::DataBdt,
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

        if let Some(referer) = &key.referer {
            let referer = BdtDataRefererInfo::decode_string(referer)?;
            // bdt回调都是chunk粒度的，所以我们需要在referer里面保存请求对应的file或者dir+inner_path
            ndn_req.object_id = referer.object_id;
            ndn_req.inner_path = referer.inner_path;

            ndn_req.common.dec_id = referer.dec_id;
            ndn_req.common.req_path = referer.req_path;
            ndn_req.common.flags = referer.flags;
            if referer.referer_object.len() > 0 {
                ndn_req.common.referer_object = referer.referer_object;
            }
        }

        let resp = self.processor.get_data(ndn_req).await;
        let ret = Self::process_resp(resp);
        self.cache.add(key, ret.clone());

        ret
    }

    async fn put_data(&self, req: BdtPutDataInputRequest) -> BuckyResult<()> {
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

        let mut ndn_req = NDNPutDataInputRequest {
            common: NDNInputRequestCommon {
                req_path: None,
                dec_id: None,
                source: req.source,
                protocol: NONProtocol::DataBdt,
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

        if let Some(referer) = &key.referer {
            let referer = BdtDataRefererInfo::decode_string(referer)?;
            ndn_req.object_id = referer.object_id;

            ndn_req.common.dec_id = referer.dec_id;
            ndn_req.common.req_path = referer.req_path;
            ndn_req.common.flags = referer.flags;
            if referer.referer_object.len() > 0 {
                ndn_req.common.referer_object = referer.referer_object;
            }
        }

        let resp = self.processor.put_data(ndn_req).await;
        let ret = Self::process_resp(resp);
        self.cache.add(key, ret.clone());

        ret
    }

    async fn delete_data(&self, req: BdtDeleteDataInputRequest) -> BuckyResult<()> {
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

        let mut ndn_req = NDNDeleteDataInputRequest {
            common: NDNInputRequestCommon {
                req_path: None,
                dec_id: None,
                source: req.source,
                protocol: NONProtocol::DataBdt,
                level: NDNAPILevel::Router,
                referer_object: vec![],
                target: None,
                flags: 0,
                user_data: None,
            },
            object_id: req.object_id,
            inner_path: None,
        };

        if let Some(referer) = &key.referer {
            let referer = BdtDataRefererInfo::decode_string(referer)?;
            ndn_req.object_id = referer.object_id;
            ndn_req.inner_path = referer.inner_path;

            ndn_req.common.dec_id = referer.dec_id;
            ndn_req.common.req_path = referer.req_path;
            ndn_req.common.flags = referer.flags;
            if referer.referer_object.len() > 0 {
                ndn_req.common.referer_object = referer.referer_object;
            }
        }

        let resp = self.processor.delete_data(ndn_req).await;
        let ret = Self::process_resp(resp);
        self.cache.add(key, ret.clone());

        ret
    }
}

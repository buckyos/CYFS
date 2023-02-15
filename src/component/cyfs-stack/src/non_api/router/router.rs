use super::super::acl::*;
use super::super::handler::*;
use super::super::non::NONOutputFailHandleProcessor;
use super::def::*;
use crate::acl::*;
use crate::forward::ForwardProcessorManager;
use crate::meta::*;
use crate::non::*;
use crate::non_api::noc::NOCLevelInputProcessor;
use crate::router_handler::RouterHandlersManager;
use crate::zone::*;
use cyfs_base::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct NONRouter {
    noc_relation: NamedObjectRelationCacheRef,

    zone_manager: ZoneManagerRef,

    // meta的处理器，处理get请求，底层会依赖noc
    meta_processor: NONInputProcessorRef,

    // 用以处理本地noc请求
    noc_raw_processor: NONInputProcessorRef,
    noc_acl_processor: NONInputProcessorRef,

    // 用以实现转发请求
    forward: ForwardProcessorManager,

    acl: AclManagerRef,

    router_handlers: RouterHandlersManager,

    fail_handler: ObjectFailHandler,
}

impl NONRouter {
    fn new(
        noc_relation: NamedObjectRelationCacheRef,

        // router内部的noc处理器，会经过acl和validate两层校验器
        noc_raw_processor: NONInputProcessorRef,

        // 用以实现转发请求
        forward: ForwardProcessorManager,
        acl: AclManagerRef,

        zone_manager: ZoneManagerRef,
        router_handlers: RouterHandlersManager,

        meta_processor: NONInputProcessorRef,
        fail_handler: ObjectFailHandler,
    ) -> NONInputProcessorRef {
        let noc_acl_processor =
            NOCLevelInputProcessor::new_rmeta_acl(acl.clone(), noc_raw_processor.clone());

        let ret = Self {
            noc_relation,

            // noc_acl_processor 带rmeta access的noc, 如果当前协议栈是router目标，那么使用此noc；
            // 如果是中间节点，那么使用raw_noc_processor来作为缓存查询
            noc_raw_processor,
            noc_acl_processor,

            forward,
            acl,

            zone_manager,

            router_handlers,

            meta_processor,
            fail_handler,
        };

        Arc::new(Box::new(ret))
    }

    pub(crate) fn new_acl(
        noc_relation: NamedObjectRelationCacheRef,
        raw_noc_processor: NONInputProcessorRef,

        // Used to forwarding requests
        forward: ForwardProcessorManager,
        acl: AclManagerRef,

        zone_manager: ZoneManagerRef,

        router_handlers: RouterHandlersManager,
        meta_processor: NONInputProcessorRef,
        fail_handler: ObjectFailHandler,
    ) -> NONInputProcessorRef {
        // router processor with rmeta acl and valdiate
        let rmeta_validate_router = Self::new(
            noc_relation,
            raw_noc_processor,
            forward,
            acl.clone(),
            zone_manager,
            router_handlers.clone(),
            meta_processor,
            fail_handler,
        );

        // Request from local rpc call or other stacks requests via bdt protocol: input->acl->pre_router->router->post_router

        // Add the router pre processor
        let pre_processor = NONHandlerPreProcessor::new(
            RouterHandlerChain::PreRouter,
            rmeta_validate_router,
            router_handlers.clone(),
        );

        // Add the router post processor
        let post_processor = NONHandlerPostProcessor::new(
            RouterHandlerChain::PostRouter,
            pre_processor,
            router_handlers,
        );

        // Wrap the processor with input acl control
        let acl_processor = NONAclInputProcessor::new(acl, post_processor.clone());

        acl_processor
    }

    async fn resolve_router_info(
        &self,
        op: AclOperation,
        source: &RequestSourceInfo,
        target: Option<&ObjectId>,
    ) -> BuckyResult<RouterHandlerRequestRouterInfo> {
        let current_info = self.zone_manager.get_current_info().await?;
        let target_zone_info = self
            .zone_manager
            .target_zone_manager()
            .resolve_target(target)
            .await?;

        // If it is the current protocol stack, then directly process on local
        let target;
        let direction;
        let next_hop;
        let next_direction;
        if target_zone_info.target_device == current_info.device_id {
            // FIXME If it is current local device, is it set to be set to the local device value, or leave it empty?
            target = None;
            direction = None;
            next_hop = None;
            next_direction = None;
        } else {
            let ret = self.next_forward_target(op, &current_info, &target_zone_info)?;
            assert!(ret.is_some());
            let next_forward_info = ret.unwrap();
            next_hop = Some(next_forward_info.0);
            next_direction = Some(next_forward_info.1);

            target = Some(target_zone_info.target_device.clone());

            if target_zone_info.is_current_zone {
                direction = Some(ZoneDirection::LocalToLocal);
            } else {
                direction = Some(ZoneDirection::LocalToRemote);
            }
        }

        Ok(RouterHandlerRequestRouterInfo {
            source: source.to_owned(),

            target,
            direction,

            next_hop,
            next_direction,
        })
    }

    // 计算下一跳的device
    fn next_forward_target(
        &self,
        op: AclOperation,
        current_info: &Arc<CurrentZoneInfo>,
        target_zone_info: &TargetZoneInfo,
    ) -> BuckyResult<Option<(DeviceId, ZoneDirection)>> {
        // 根据当前设备是不是ood，需要区别对待：
        let forward_target = if current_info.zone_role.is_ood_device() {
            // 同zone，那么直接转发到目标device
            if target_zone_info.is_current_zone {
                (
                    target_zone_info.target_device.clone(),
                    ZoneDirection::LocalToLocal,
                )
            } else {
                // 不同zone，那么转发到目标device所在zone ood
                (
                    target_zone_info.target_ood.clone(),
                    ZoneDirection::LocalToRemote,
                )
            }
        } else {
            // 判断是不是就是自己
            if target_zone_info.target_device == current_info.device_id {
                (
                    target_zone_info.target_device.clone(),
                    ZoneDirection::LocalToLocal,
                )
            } else {
                // 判断是不是需要绕过当前zone的ood设备，直接发送请求到目标ood
                let bypass_current_ood = match op.category() {
                    AclOperationCategory::Read => self.acl.config().read_bypass_ood,
                    AclOperationCategory::Write => self.acl.config().write_bypass_ood,
                    AclOperationCategory::Both => unreachable!(),
                };

                // 非自己，并且自己也非ood设备，需要根据策略，转发到当前zone ood或者目标zone ood处理
                if bypass_current_ood {
                    if target_zone_info.is_current_zone {
                        // 同zone，那么直接发送到目标设备
                        (
                            target_zone_info.target_device.clone(),
                            ZoneDirection::LocalToLocal,
                        )
                    } else {
                        // 不同zone，那么直接发送到目标zone ood
                        (
                            target_zone_info.target_ood.clone(),
                            ZoneDirection::LocalToRemote,
                        )
                    }
                } else {
                    // 非bypass情况下，必须发送到当前ood
                    (
                        current_info.zone_device_ood_id.clone(),
                        ZoneDirection::LocalToLocal,
                    )
                }
            }
        };

        // 如果转发目标就是当前设备，那么不需要转发了
        let ret = if forward_target.0 == current_info.device_id {
            None
        } else {
            Some(forward_target)
        };

        Ok(ret)
    }

    async fn get_forward(&self, target: &DeviceId) -> BuckyResult<NONInputProcessorRef> {
        // 获取到目标的processor
        let requestor = self.forward.get(target).await?;

        // 这里不指定dec_id，使用forward request里面的dec_id
        let processor = NONRequestor::new(None, requestor).into_processor();

        // 增加一层错误监测处理
        let processor =
            NONOutputFailHandleProcessor::new(target.clone(), self.fail_handler.clone(), processor);

        // 转换为input processor
        let input_processor = NONInputTransformer::new(processor);

        // 增加forward前置处理器
        let pre_processor = NONHandlerPreProcessor::new(
            RouterHandlerChain::PreForward,
            input_processor,
            self.router_handlers.clone(),
        );

        // 增加forward后置处理器
        let post_processor = NONHandlerPostProcessor::new(
            RouterHandlerChain::PostForward,
            pre_processor,
            self.router_handlers.clone(),
        );

        Ok(post_processor)
    }

    async fn default_put_object(
        &self,
        router_info: &RouterHandlerRequestRouterInfo,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        info!(
            "router put_object default handler, object={}, router={}",
            req.object.object_id, router_info,
        );

        assert!(req.common.source.is_current_zone());

        // put-object只在目标设备上保存
        if router_info.next_hop.is_none() {
            // 没有下一跳了，说明已经到达目标设备

            let object_id = req.object.object_id.clone();
            let put_ret = self.noc_acl_processor.put_object(req).await;
            if put_ret.is_err() {
                error!(
                    "router put_object to noc but failed! object={}, {}",
                    object_id,
                    put_ret.as_ref().unwrap_err()
                );
            }

            return put_ret;
        }

        // 不再修正req的target
        assert!(router_info.target.is_some());

        /*
        debug!(
            "will forward put object: target={}, direction={}",
            forward_target, direction
        );
        */
        let forward_processor = self
            .get_forward(router_info.next_hop.as_ref().unwrap())
            .await?;

        forward_processor.put_object(req).await.map_err(|mut e| {
            // 需要区分一下是zone内连接失败，还是跨zone连接失败
            if e.code() == BuckyErrorCode::ConnectFailed
                && *router_info.next_direction.as_ref().unwrap() == ZoneDirection::LocalToRemote
            {
                e.set_code(BuckyErrorCode::ConnectInterZoneFailed);
            }
            e
        })
    }

    // 首先需要本地保存到noc
    // 如果当前device不是zone的主ood，那么需要发送到ood上保存
    // 如果保存目标不是当前device，也不是主ood，那么ood负责发送到目标device，可能会跨zone
    pub async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        info!("will handle router put_object request: {}", req);

        let router_info = self
            .resolve_router_info(
                AclOperation::PutObject,
                &req.common.source,
                req.common.target.as_ref(),
            )
            .await?;

        self.default_put_object(&router_info, req).await
    }

    // 从meta查询，目前只有主ood才会从meta查询(其余device查询会导致冗余操作？)
    async fn get_from_meta(
        &self,
        req: &NONGetObjectInputRequest,
    ) -> BuckyResult<Option<NONGetObjectInputResponse>> {
        match self.meta_processor.get_object(req.clone()).await {
            Ok(resp) => Ok(Some(resp)),
            Err(e) => match e.code() {
                BuckyErrorCode::NotFound => {
                    debug!(
                        "get object from meta chain but not found! {}",
                        req.object_id
                    );

                    Ok(None)
                }
                _ => {
                    warn!("get object from meta chain error! {}, {}", req.object_id, e);

                    Err(e)
                }
            },
        }
    }

    // 对于(dir | objectmap) + inner_path的请求，返回InnerPathNotFound说明对象已经查找到，但指定的内部路径不存在，所以不需要继续后续路由了
    fn is_dir_inner_path_error(req: &NONGetObjectInputRequest, e: &BuckyError) -> bool {
        let type_code = req.object_id.obj_type_code();
        if req.inner_path.is_some()
            && e.code() == BuckyErrorCode::InnerPathNotFound
            && (type_code == ObjectTypeCode::Dir || type_code == ObjectTypeCode::ObjectMap)
        {
            true
        } else {
            false
        }
    }

    async fn default_get_object(
        &self,
        save_to_noc: bool,
        router_info: &RouterHandlerRequestRouterInfo,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        info!(
            "router get_object default handler, req={}, router={}",
            req, router_info,
        );

        // 如果开启了flush标志位，那么不首先从noc获取
        let flush = (req.common.flags & CYFS_ROUTER_REQUEST_FLAG_FLUSH) != 0;

        let mut err = None;
        if flush {
            match self
                .get_object_without_noc(save_to_noc, router_info, req.clone())
                .await
            {
                Ok(resp) => return Ok(resp),
                Err(e) => err = Some(e),
            }
        }

        // 选择正确的noc处理器
        let noc_processor = if router_info.next_hop.is_none() {
            // 目标协议栈，那么需要校验rmeta access
            Some(&self.noc_acl_processor)
        } else {
            if req.common.source.zone.is_current_zone() || req.common.source.zone.is_friend_zone() {
                // 中间节点，noc作为缓存处理，直接使用object层的acl来处理
                Some(&self.noc_raw_processor)
            } else {
                // 如果是跨设备的查找，并且来源是other zone，那么绕过缓存，直接朝目标发起
                None
            }
        };

        // 从本地noc查询
        if let Some(noc_processor) = noc_processor {
            match noc_processor.get_object(req.clone()).await {
                Ok(resp) => {
                    if let Some(next) = &router_info.next_hop {
                        info!(
                            "router get_object from local noc cache! id={}, next_hop={}",
                            req.object_id, next
                        );
                    } else {
                        info!("router get_object from local noc! id={}", req.object_id,);
                    }

                    return Ok(resp);
                }
                Err(e) => {
                    if Self::is_dir_inner_path_error(&req, &e) {
                        return Err(e);
                    }

                    if RouterHandlerAction::is_action_error(&e) {
                        warn!(
                            "get object from noc stopped by action: obj={}, {}",
                            req.object_id, e
                        );
                        return Err(e);
                    }

                    if e.code() == BuckyErrorCode::PermissionDenied {
                        warn!(
                            "get object from noc stopped by access: obj={}, {}",
                            req.object_id, e
                        );
                        return Err(e);
                    }
                }
            }
        }

        if !flush {
            self.get_object_without_noc(save_to_noc, router_info, req)
                .await
        } else {
            Err(err.unwrap())
        }
    }

    async fn get_object_without_noc(
        &self,
        save_to_noc: bool,
        router_info: &RouterHandlerRequestRouterInfo,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        let current_info = self.zone_manager.get_current_info().await?;

        // 1. 如果当前是zone的ood，那么尝试从meta_chain查找
        // 2. 如果配置了bypass_ood，那么也需要尝试从meta_chain查找
        if current_info.zone_role.is_ood_device() || self.acl.config().read_bypass_ood {
            match self.get_from_meta(&req).await {
                Ok(Some(resp)) => {
                    return Ok(resp);
                }
                Ok(None) => {}
                Err(e) => {
                    if Self::is_dir_inner_path_error(&req, &e) {
                        return Err(e);
                    }

                    /*
                    如果只是被meta_processor拒绝，那么不应该影响整体流程
                    if RouterHandlerAction::is_action_error(&e) {
                        warn!(
                            "router get object stopped from meta by action: obj={}, {}",
                            req.object_id, e
                        );
                        return Err(e);
                    }
                    */
                }
            }
        }

        if router_info.next_hop.is_none() {
            // 没有下一跳了，那么返回失败
            let msg =
                format!(
                "router get object from final target but not found: obj={}, source={}, device={}",
                req.object_id, req.common.source, self.zone_manager.get_current_device_id(),
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        // 不再修正req的target，保留请求原始值
        assert!(router_info.target.is_some());
        // req.target = router_info.target.clone().map(|o| o.into());

        /*
        info!(
            "will forward get object: target={}, direction={}",
            forward_target, direction
        );
        */

        let forward_processor = self
            .get_forward(router_info.next_hop.as_ref().unwrap())
            .await?;

        let ret = forward_processor
            .get_object(req.clone())
            .await
            .map_err(|mut e| {
                // 需要区分一下是zone内连接失败，还是跨zone连接失败
                if e.code() == BuckyErrorCode::ConnectFailed
                    && *router_info.next_direction.as_ref().unwrap() == ZoneDirection::LocalToRemote
                {
                    e.set_code(BuckyErrorCode::ConnectInterZoneFailed);
                }
                e
            });

        // Try to cache relation
        if req.is_with_inner_path_relation() {
            let cache_key = NamedObjectRelationCacheKey {
                object_id: req.object_id.clone(),
                relation_type: NamedObjectRelationType::InnerPath,
                relation: req.inner_path.as_ref().unwrap().clone(),
            };

            let req = match &ret {
                Ok(resp) => Some(NamedObjectRelationCachePutRequest {
                    cache_key,
                    target_object_id: Some(resp.object.object_id.clone()),
                }),
                Err(e)
                    if e.code() == BuckyErrorCode::NotFound
                        || e.code() == BuckyErrorCode::InnerPathNotFound =>
                {
                    Some(NamedObjectRelationCachePutRequest {
                        cache_key,
                        target_object_id: None,
                    })
                }
                _ => None,
            };

            if let Some(req) = req {
                let relation = self.noc_relation.clone();
                async_std::task::spawn(async move {
                    let _ = relation.put(&req).await;
                });
            }
        }

        // 通过forward查询成功后，本地需要缓存
        if ret.is_ok() && save_to_noc {
            let put_req = NONPutObjectInputRequest {
                common: req.common.clone(),
                object: ret.as_ref().unwrap().object.clone(),
                access: None,
            };

            let _r = self.noc_raw_processor.put_object(put_req).await;
        }

        ret
    }

    pub async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        // info!("router will handle get_object request: {}", req);
        // 查找操作一定会转发到当前zone的ood来处理
        // final_target有下面几种情况，查找流程如下(其中-->表示跨协议栈转发操作)
        // 1. 当前协议栈： noc-->ood->noc->meta
        // 2. 当前zone的主ood：noc->meta
        // 3. 当前zone的其它ood：noc-->ood->noc->meta-->device->noc
        // 4. 其它zone的ood：noc-->ood->noc->meta-->device's ood->noc->meta-->device->noc

        // TODO 目前不支持object的溯源查找，这个需要保存object来源信息后才可以支持

        let router_info = self
            .resolve_router_info(
                AclOperation::GetObject,
                &req.common.source,
                req.common.target.as_ref(),
            )
            .await?;

        self.default_get_object(true, &router_info, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        info!("router will handle post object request: {}", req);

        let router_info = self
            .resolve_router_info(
                AclOperation::PostObject,
                &req.common.source,
                req.common.target.as_ref(),
            )
            .await?;

        let object_id = std::borrow::Cow::Borrowed(&req.object.object_id);
        if router_info.next_hop.is_none() {
            // 没有下一跳了，交由handler处理器
            return self.noc_acl_processor.post_object(req).await;
        }

        // 不再修正req的target，保留请求原始值
        assert!(router_info.target.is_some());

        info!(
            "will forward post object: req={}, router={}",
            req.object.object_id, router_info,
        );

        let forward_processor = self
            .get_forward(router_info.next_hop.as_ref().unwrap())
            .await?;

        let object_id = object_id.into_owned();
        forward_processor
            .post_object(req)
            .await
            .map_err(|mut e| {
                // 需要区分一下是zone内连接失败，还是跨zone连接失败
                if e.code() == BuckyErrorCode::ConnectFailed
                    && *router_info.next_direction.as_ref().unwrap() == ZoneDirection::LocalToRemote
                {
                    e.set_code(BuckyErrorCode::ConnectInterZoneFailed);
                }
                e
            })
            .map(|resp| {
                info!(
                    "forward post object response: req={}, resp={}",
                    object_id, resp
                );
                resp
            })
    }

    pub async fn select_object(
        &self,
        _req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        let msg = format!("select_object is no longer supported!");
        error!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
    }

    async fn default_delete_object(
        &self,
        router_info: &RouterHandlerRequestRouterInfo,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        // delete_object应该只同zone访问
        assert!(req.common.source.is_current_zone());

        // delete_object只在目标设备删除object
        if router_info.next_hop.is_none() {
            assert!(router_info.target.is_none());

            // 没有下一跳了，说明已经到达目标设备
            // 直接从当前设备删除即可
            let object_id = req.object_id.clone();
            let delete_ret = self.noc_acl_processor.delete_object(req).await;
            if delete_ret.is_err() {
                error!(
                    "router delete_object from noc but failed! object={}, {}",
                    object_id,
                    delete_ret.as_ref().unwrap_err()
                );
            }

            return delete_ret;
        }

        // 不再修正req的target
        assert!(router_info.target.is_some());

        info!(
            "will forward delete object: target={}, direction={}",
            router_info.next_hop.as_ref().unwrap(),
            router_info.next_direction.as_ref().unwrap()
        );

        let forward_processor = self
            .get_forward(router_info.next_hop.as_ref().unwrap())
            .await?;

        forward_processor.delete_object(req).await.map_err(|mut e| {
            // 需要区分一下是zone内连接失败，还是跨zone连接失败
            if e.code() == BuckyErrorCode::ConnectFailed
                && *router_info.next_direction.as_ref().unwrap() == ZoneDirection::LocalToRemote
            {
                e.set_code(BuckyErrorCode::ConnectInterZoneFailed);
            }
            e
        })
    }

    pub async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        info!("will handle router delete_object request: {}", req);

        // final_target有下面几种情况
        // 1. 当前zone：从ood和路径device删除
        // 2. 其它zone，那么直接转发到目标zone ood，当前zone的设备不触发任何删除操作

        let router_info = self
            .resolve_router_info(
                AclOperation::GetObject,
                &req.common.source,
                req.common.target.as_ref(),
            )
            .await?;

        self.default_delete_object(&router_info, req).await
    }
}

#[async_trait::async_trait]
impl NONInputProcessor for NONRouter {
    async fn put_object(
        &self,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        NONRouter::put_object(&self, req).await
    }

    async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        NONRouter::get_object(&self, req).await
    }

    async fn post_object(
        &self,
        req: NONPostObjectInputRequest,
    ) -> BuckyResult<NONPostObjectInputResponse> {
        NONRouter::post_object(&self, req).await
    }

    async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        NONRouter::select_object(&self, req).await
    }

    async fn delete_object(
        &self,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        NONRouter::delete_object(&self, req).await
    }
}

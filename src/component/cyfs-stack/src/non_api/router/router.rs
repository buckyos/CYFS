use super::super::acl::*;
use super::super::handler::*;
use super::super::non::NONOutputFailHandleProcessor;
use super::def::*;
use super::handler::NONRouterHandler;
use crate::acl::*;
use crate::forward::ForwardProcessorManager;
use crate::meta::*;
use crate::non::*;
use crate::router_handler::RouterHandlersManager;
use crate::zone::*;
use cyfs_base::*;
use cyfs_core::ZoneId;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct NONRouter {
    zone_manager: ZoneManager,

    // meta的处理器，处理get请求，底层会依赖noc
    meta_processor: NONInputProcessorRef,

    // 用以处理本地noc请求
    noc_processor: NONInputProcessorRef,

    // 用以实现转发请求
    forward: ForwardProcessorManager,

    acl: AclManagerRef,

    router_handlers: RouterHandlersManager,

    fail_handler: ObjectFailHandler,

    // action's handler with router handler system, now only valid for post_object
    handler: Arc<NONRouterHandler>,
}

impl NONRouter {
    fn new_raw(
        // router内部的noc不带任何权限
        raw_noc_processor: NONInputProcessorRef,

        // 用以实现转发请求
        forward: ForwardProcessorManager,
        acl: AclManagerRef,

        zone_manager: ZoneManager,

        router_handlers: RouterHandlersManager,

        meta_processor: NONInputProcessorRef,
        fail_handler: ObjectFailHandler,
    ) -> NONInputProcessorRef {
        let handler = NONRouterHandler::new(&router_handlers, zone_manager.clone());

        let ret = Self {
            noc_processor: raw_noc_processor,
            forward,
            acl,

            zone_manager,

            router_handlers,

            meta_processor,
            fail_handler,

            handler: Arc::new(handler),
        };

        Arc::new(Box::new(ret))
    }

    pub(crate) fn new_acl(
        raw_noc_processor: NONInputProcessorRef,

        // 用以实现转发请求
        forward: ForwardProcessorManager,
        acl: AclManagerRef,

        zone_manager: ZoneManager,

        router_handlers: RouterHandlersManager,
        meta_processor: NONInputProcessorRef,
        fail_handler: ObjectFailHandler,
    ) -> NONInputProcessorRef {
        // 不带input acl的处理器
        let raw_router = Self::new_raw(
            raw_noc_processor,
            forward,
            acl.clone(),
            zone_manager,
            router_handlers.clone(),
            meta_processor,
            fail_handler,
        );

        // 入栈的请求，依次是 input->acl->pre_router->router->post_router
        // 本地发起的请求，直接进入router: input->pre_router->router->post_router

        // 增加router前置处理器
        let pre_processor = NONHandlerPreProcessor::new(
            RouterHandlerChain::PreRouter,
            raw_router,
            router_handlers.clone(),
        );

        // 增加router后置处理器
        let post_processor = NONHandlerPostProcessor::new(
            RouterHandlerChain::PostRouter,
            pre_processor,
            router_handlers,
        );

        // 带控制input acl权限的处理器
        let acl_router = NONAclInputProcessor::new(acl, post_processor.clone());

        // 使用acl switcher连接(本地调用不经过acl)
        let processor = NONInputAclSwitcher::new(acl_router, post_processor);

        processor
    }

    async fn resolve_router_info(
        &self,
        op: AclOperation,
        source: &DeviceId,
        target: Option<&ObjectId>,
    ) -> BuckyResult<RouterHandlerRequestRouterInfo> {
        let current_info = self.zone_manager.get_current_info().await?;
        let (target_zone_id, final_target) = self.zone_manager.resolve_target(target, None).await?;

        // 如果就是当前协议栈，那么直接从noc select
        let target;
        let direction;
        let next_hop;
        let next_direction;
        if final_target == current_info.device_id {
            // FIXME 如果是本地device，这里是设置为本地device值，还是都置空？
            target = None;
            direction = None;
            next_hop = None;
            next_direction = None;
        } else {
            let ret =
                self.next_forward_target(op, &current_info, &target_zone_id, &final_target)?;
            assert!(ret.is_some());
            let next_forward_info = ret.unwrap();
            next_hop = Some(next_forward_info.0);
            next_direction = Some(next_forward_info.1);

            target = Some(final_target.clone());

            if target_zone_id == current_info.zone_id {
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
        target_zone_id: &ZoneId,
        final_target: &DeviceId,
    ) -> BuckyResult<Option<(DeviceId, ZoneDirection)>> {
        // 根据当前设备是不是ood，需要区别对待：
        let forward_target = if current_info.zone_role.is_ood_device() {
            // 同zone，那么直接转发到目标device
            if *target_zone_id == current_info.zone_id {
                (final_target.clone(), ZoneDirection::LocalToLocal)
            } else {
                // 不同zone，那么转发到目标device所在zone ood
                (
                    self.zone_manager.get_zone_ood(target_zone_id)?,
                    ZoneDirection::LocalToRemote,
                )
            }
        } else {
            // 判断是不是就是自己
            if *final_target == current_info.device_id {
                (final_target.clone(), ZoneDirection::LocalToLocal)
            } else {
                // 判断是不是需要绕过当前zone的ood设备，直接发送请求到目标ood
                let bypass_current_ood = match op.category() {
                    AclOperationCategory::Read => self.acl.config().read_bypass_ood,
                    AclOperationCategory::Write => self.acl.config().write_bypass_ood,
                    AclOperationCategory::Both => unreachable!(),
                };

                // 非自己，并且自己也非ood设备，需要根据策略，转发到当前zone ood或者目标zone ood处理
                if bypass_current_ood {
                    if *target_zone_id == current_info.zone_id {
                        // 同zone，那么直接发送到目标设备
                        (final_target.clone(), ZoneDirection::LocalToLocal)
                    } else {
                        // 不同zone，那么直接发送到目标zone ood
                        (
                            self.zone_manager.get_zone_ood(target_zone_id)?,
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

        // 标准acl output权限
        let processor = NONAclOutputProcessor::new(
            NONProtocol::HttpBdt,
            self.acl.clone(),
            target.to_owned(),
            processor,
        );

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
        save_to_noc: bool,
        router_info: &RouterHandlerRequestRouterInfo,
        req: NONPutObjectInputRequest,
    ) -> BuckyResult<NONPutObjectInputResponse> {
        info!(
            "router put_object default handler, object={}, router={}, save={}",
            req.object.object_id, router_info, save_to_noc,
        );

        let put_ret = if save_to_noc {
            // 首先保存到本地noc，这里如果失败如何处理？
            let ret = self.noc_processor.put_object(req.clone()).await;
            if let Err(e) = &ret {
                // 如果noc路径上被拒绝了，那么终止路由
                if e.code() == BuckyErrorCode::Ignored || e.code() == BuckyErrorCode::Reject {
                    return Err(ret.unwrap_err());
                } else {
                    // TODO 保存到noc失败了，那么RouterPutObjectResult要返回什么？
                }
            }
            ret
        } else {
            // FIXME 如果最终处理点也没保存到noc，那么是返回什么？
            Err(BuckyError::from(BuckyErrorCode::Ignored))
        };

        if router_info.next_hop.is_none() {
            // 没有下一跳了，说明已经到达目标设备
            return put_ret;
        }

        // 不再修正req的target
        assert!(router_info.target.is_some());
        // req.target = router_info.target.clone().map(|o| o.into());

        // final_target有下面几种情况
        // 1. 当前协议栈： 保存到当前noc，转发到当前zone主ood并保存
        // 2. 当前zone的主ood：保存noc，转发到当前zone主ood并保存
        // 3. 当前zone的其它device：保存noc，转发到当前zone主ood并保存，主ood再转发到同zone的目标device
        // 4. 其它zone的ood：保存noc，转发到当前zone主ood并保存，主ood再转发到目标zone ood, 目标zone ood再转发到目标target

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

        self.default_put_object(true, &router_info, req).await
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

    // 对于dir+inner_path的请求，返回InnerPathNotFound说明对象已经查找到，但指定的内部路径不存在，所以不需要继续后续路由了
    fn is_dir_inner_path_error(req: &NONGetObjectInputRequest, e: &BuckyError) -> bool {
        if e.code() == BuckyErrorCode::InnerPathNotFound
            && req.object_id.obj_type_code() == ObjectTypeCode::Dir
            && req.inner_path.is_some()
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
            "router get_object default handler, object={}, router={}",
            req.object_id, router_info,
        );

        // 如果开启了flush标志位，那么不首先从noc获取
        let flush = (req.common.flags & CYFS_ROUTER_REQUEST_FLAG_FLUSH) != 0;

        if flush {
            if let Ok(resp) = self
                .get_object_without_noc(save_to_noc, router_info, req.clone())
                .await
            {
                return Ok(resp);
            }
        }

        // 从本地noc查询
        match self.noc_processor.get_object(req.clone()).await {
            Ok(resp) => {
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
            }
        }

        if !flush {
            self.get_object_without_noc(save_to_noc, router_info, req)
                .await
        } else {
            Err(BuckyError::from(BuckyErrorCode::NotFound))
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

        let resp = forward_processor
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
            })?;

        // 通过forward查询成功后，本地需要缓存
        if save_to_noc {
            let put_req = NONPutObjectInputRequest {
                common: req.common.clone(),
                object: resp.object.clone(),
            };

            let _r = self.noc_processor.put_object(put_req).await;
        }

        Ok(resp)
    }

    async fn put_to_noc_on_get_object_handler_resp(
        &self,
        resp: &NONGetObjectInputResponse,
        req: NONGetObjectInputRequest,
    ) {
        info!(
            "router will save object to noc on get object handler resp: obj={}, dec={:?}",
            resp.object.object_id, req.common.dec_id
        );

        let put_req = NONPutObjectInputRequest {
            common: req.common,
            object: resp.object.clone(),
        };

        let _r = self.noc_processor.put_object(put_req).await;
    }

    pub async fn get_object(
        &self,
        req: NONGetObjectInputRequest,
    ) -> BuckyResult<NONGetObjectInputResponse> {
        info!("will handle router get_object request: {}", req);
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
            return self.handler.post_object(req).await;
        }

        // 不再修正req的target，保留请求原始值
        assert!(router_info.target.is_some());

        /*
        info!(
            "will forward post object: target={}, direction={}",
            forward_target, direction
        );
        */

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
                info!("post_object response: req={}, resp={}", object_id, resp);
                resp
            })
    }

    async fn default_select_object(
        &self,
        save_to_noc: bool,
        router_info: &RouterHandlerRequestRouterInfo,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        info!(
            "will select: filter={:?}, opt={:?}, source={}, target={:?}",
            req.filter, req.opt, req.common.source, req.common.target,
        );

        // select操作必须明确在一台device(协议栈)上发生，目前不支持合并
        // 1. target没指定，那么当前zone ood
        // 2. target明确指定了device，那么根据device是不是同zone，路径分别为
        //        同zone: local->ood->target
        //        不同zone: local->ood->target_ood->target
        // 3. target指定了zone(people)，那么向zone主ood发起，但根据zone是不是当前zone，路径分别为
        //        同zone: local->ood
        //        不同zone: local->ood->target_ood

        // 1. 如果明确指定了device，那么需要转发到目标device上select，需要通过zone的ood转发(即使是同zone)
        // 2. 没有明确指定device, 那么需要在目标zone主ood上select

        // 如果就是当前协议栈，那么直接从noc select
        if router_info.next_hop.is_none() {
            return self.noc_processor.select_object(req).await.map_err(|e| {
                error!("router select object from noc failed! {}", e);
                e
            });
        }

        // 不再修正req的target
        assert!(router_info.target.is_some());
        // req.common.target = router_info.target.clone().map(|o| o.into());

        let forward_processor = self
            .get_forward(router_info.next_hop.as_ref().unwrap())
            .await?;

        let req_common = if save_to_noc {
            Some(req.common.clone())
        } else {
            None
        };

        let select_resp = forward_processor
            .select_object(req)
            .await
            .map_err(|mut e| {
                // 需要区分一下是zone内连接失败，还是跨zone连接失败
                if e.code() == BuckyErrorCode::ConnectFailed
                    && *router_info.next_direction.as_ref().unwrap() == ZoneDirection::LocalToRemote
                {
                    e.set_code(BuckyErrorCode::ConnectInterZoneFailed);
                }
                e
            })?;

        if save_to_noc {
            let common = req_common.unwrap();
            // TODO 这里的source是不是要更新?
            // common.source = router_info.target.clone();
            self.cache_select_result(&common, &select_resp).await;
        }

        Ok(select_resp)
    }

    async fn cache_select_result(
        &self,
        common: &NONInputRequestCommon,
        resp: &NONSelectObjectInputResponse,
    ) {
        let mut req_list = Vec::new();
        for item in &resp.objects {
            let object = item.object.as_ref().unwrap();

            let req = NONPutObjectInputRequest {
                common: common.clone(),
                object: object.to_owned(),
            };

            req_list.push(req);
        }

        let noc = self.noc_processor.clone();
        async_std::task::spawn(async move {
            for req in req_list {
                let object_id = req.object.object_id.clone();
                if let Err(e) = noc.put_object(req).await {
                    if e.code() == BuckyErrorCode::AlreadyExists {
                        info!("object alreay in noc: {}", object_id);
                    } else if e.code() == BuckyErrorCode::AlreadyExistsAndSignatureMerged {
                        info!("object alreay in noc and signs updated: {}", object_id);
                    } else {
                        error!("insert object to local cache error: {} {}", object_id, e);
                    }
                } else {
                    info!("cache select object success: {}", object_id);
                }
            }
        });
    }

    pub async fn select_object(
        &self,
        req: NONSelectObjectInputRequest,
    ) -> BuckyResult<NONSelectObjectInputResponse> {
        info!("will handle router select_object request: {}", req);

        // select操作必须明确在一台device(协议栈)上发生，目前不支持合并
        // 1. target没指定，那么当前zone ood
        // 2. target明确指定了device，那么根据device是不是同zone，路径分别为
        //        同zone: local->ood->target
        //        不同zone: local->ood->target_ood->target
        // 3. target指定了zone(people)，那么向zone主ood发起，但根据zone是不是当前zone，路径分别为
        //        同zone: local->ood
        //        不同zone: local->ood->target_ood

        let router_info = self
            .resolve_router_info(
                AclOperation::SelectObject,
                &req.common.source,
                req.common.target.as_ref(),
            )
            .await?;

        self.default_select_object(true, &router_info, req).await
    }

    async fn default_delete_object(
        &self,
        router_info: &RouterHandlerRequestRouterInfo,
        req: NONDeleteObjectInputRequest,
    ) -> BuckyResult<NONDeleteObjectInputResponse> {
        // finnal_target有下面几种情况
        // 1. 当前zone：从ood和路径device删除
        // 2. 其它zone，那么直接转发到目标zone ood，当前zone的设备不触发任何删除操作

        if router_info.next_hop.is_none() {
            assert!(router_info.target.is_none());
            // 没有下一跳了，说明已经到达目标设备
            // 直接从当前设备删除即可
            return self.noc_processor.delete_object(req).await;
        }

        // 如果当前是目标zone，那么路径上的设备，不管是ood还是device，都要触发删除操作
        if *router_info.direction.as_ref().unwrap() == ZoneDirection::LocalToLocal
            || *router_info.direction.as_ref().unwrap() == ZoneDirection::RemoteToLocal
        {
            let noc_ret = self.noc_processor.delete_object(req.clone()).await;
            if let Err(e) = &noc_ret {
                // 如果noc路径上被拒绝了，那么终止路由
                if e.code() == BuckyErrorCode::Ignored || e.code() == BuckyErrorCode::Reject {
                    return noc_ret;
                }
            }
        }

        // 不再修正req的target
        assert!(router_info.target.is_some());
        // req.common.target = router_info.target.clone().map(|o| o.into());

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

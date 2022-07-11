use cyfs_base::*;
use cyfs_perf_base::*;
use log::*;
use cyfs_lib::*;
use async_trait::async_trait;
use cyfs_core::{CoreObjectType};
use std::sync::{Arc};
use std::str::FromStr;
use crate::storage::*;
use cyfs_util::EventListenerAsyncRoutine;

use crate::perf_manager::{PERF_MANAGER};

pub const PERF_DEC_ID_STR: &str = "5aSixgP8EPf6HkP54Qgybddhhsd1fgrkg7Atf2icJiiS";

//#[derive(Debug)]
pub struct DeviceInfo {
    pub ood_id: DeviceId,
    pub owner_id: PeopleId,
    pub dec_id: ObjectId,
}

impl DeviceInfo {
    pub(crate) fn new(owner_id: PeopleId, ood_id: DeviceId, dec_id: ObjectId) -> DeviceInfo {
        DeviceInfo {
            ood_id,
            owner_id,
            dec_id
        }
    }
}

struct OnPrePutRouter {
    owner: Arc<PerfService>,
}

struct OnPostPutRouter {
    owner: Arc<PerfService>,
}

#[async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult> for OnPostPutRouter {
    async fn call(&self, param: &RouterHandlerPutObjectRequest) -> BuckyResult<RouterHandlerPutObjectResult> {

        let result = RouterHandlerPutObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };

        let owned_param = (param.request).clone();
        info!("router event on_pre_put_to_router: {}", &owned_param.object.object_id);

        return Ok(result);
    }
}

#[async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPutObjectRequest, RouterHandlerPutObjectResult> for OnPrePutRouter {
    async fn call(&self, param: &RouterHandlerPutObjectRequest) -> BuckyResult<RouterHandlerPutObjectResult> {
        let mut result = RouterHandlerPutObjectResult {
            action: RouterHandlerAction::Pass,
            request: None,
            response: None,
        };
        // // 这里要处理不是自己put到noc的对象
        // if param.request.common.source == self.owner.device_info.ood_id {
        //     return Ok(result);
        // }

        // if let Some(dec_id) = param.request.common.dec_id {
        //     if dec_id != self.owner.device_info.dec_id {
        //         return Ok(result);
        //     }
        // }

        let owner = self.owner.clone();
        let owned_param = (param.request).clone();

        // 验证对象签名，决定是否保存
        if self.owner.verify_object(&param.request).await {
            // 这里是实际处理流程，不占用路由时间
            async_std::task::spawn(async move {
                info!("router event on_post_put_to_noc: {}", &owned_param.object.object_id);
                let owner2 = owner.clone();
                let _ = owner.on_put(&owned_param, owner2).await;
            });
            result.action = RouterHandlerAction::Pass;
            Ok(result)
        } else {
            info!("reject object {}", &param.request.object.object_id);
            result.action = RouterHandlerAction::Reject;
            Ok(result)
        }
    }
}

//#[derive(Debug)]
pub struct PerfService {
    pub(crate) device_info: DeviceInfo,
    cyfs_stack: SharedCyfsStack,
    auto_rebuild: bool,
}

impl PerfService {
    pub fn new(cyfs_stack: SharedCyfsStack, auto_rebuild: bool) -> Self {
        let device_id = cyfs_stack.local_device_id().clone();
        let owner_id = PeopleId::default();

        let dec_id = ObjectId::from_str(PERF_DEC_ID_STR).unwrap();

        let device_info = DeviceInfo::new(owner_id, device_id, dec_id);

        info!("device {}, owner {}, dec {}", &device_info.ood_id, &device_info.owner_id, &device_info.dec_id);
        Self {
            cyfs_stack: cyfs_stack.clone(),
            device_info,
            auto_rebuild,
        }
    }


    pub async fn init(&mut self) {
        
        let mut perf_manager = PERF_MANAGER.lock().unwrap();
        let _ = perf_manager.init(&StorageType::MangoDB, "perf-service").await;
    }

    pub fn start(service: Arc<PerfService>) {

        // 注册on_post_put_router事件
        let listener = OnPrePutRouter {
            owner: service.clone(),
        };

        // 只监听应用自己的DecObject
        let filter = format!("obj_type == {} && protocol != http-local", CoreObjectType::PerfOperation as u16);
        service.cyfs_stack
            .router_handlers()
            .add_handler(
                RouterHandlerChain::PreRouter,
                "cyfs_perf_on_pre_put_router",
                0,
                &filter,
                RouterHandlerAction::Default,
                Some(Box::new(listener)))
            .unwrap();

        let listener2 = OnPostPutRouter {
            owner: service.clone(),
        };

        service.cyfs_stack
            .router_handlers()
            .add_handler(
                RouterHandlerChain::PostRouter,
                "cyfs_perf_on_post_put_router",
                0,
                &filter,
                RouterHandlerAction::Default,
                Some(Box::new(listener2)))
            .unwrap();

    }

    pub async fn on_put(&self, req: &NONPutObjectInputRequest, _owner: Arc<PerfService>) -> BuckyResult<()> {
        info!("######on put {}, from:{}", &req.object.object_id, req.common.source);
        if let Some(object) = req.object.object.as_ref() {
            match object.as_ref() {
                AnyNamedObject::Standard(_obj) => Ok(()),
                AnyNamedObject::Core(obj) => match CoreObjectType::from(obj.desc().obj_type()) {
                    CoreObjectType::PerfOperation => self.on_perf(req).await,
                    _ => Ok(()),
                },
                AnyNamedObject::DECApp(_obj) => Ok(()),
            }
        } else {
            error!("{}", "not valid object");
            Err(BuckyError::from("not valid object"))
        }
    }

    // 这里验证对象签名是否正确，验证正确的对象才会被保存
    // 这里也验证对象类型，无关对象返回Pass，有关对象返回Accept或者Reject
    pub async fn verify_object(&self, req: &NONPutObjectInputRequest) -> bool {
        // 这里用业务逻辑检查
        match req.object.object.as_ref().unwrap().as_ref() {
            _ => true
        }
    }

    async fn on_perf(&self, req: &NONPutObjectInputRequest) -> BuckyResult<()> {

        let perf = Perf::clone_from_slice(&req.object.object_raw).unwrap();

        let obj_owner = perf.desc().owner().unwrap();

        info!(
            "###### {} recv msg {}. from:{}, owner:{}, people:{}, device:{}, dec_id: {}, id: {}",
            &self.device_info.owner_id,
            &req.object.object_id,
            req.common.source,
            obj_owner,
            perf.people(),
            perf.device(),
            perf.dec_id(),
            perf.get_id()
        );

        let all = perf.get_entity_list();

        //TODO: 保存到root state

        let perf_manager = PERF_MANAGER.lock().unwrap().clone();
        let _ = perf_manager.insert_entity_list(perf.people(),
                                        perf.device(),
                                        perf.dec_id().to_string(),
                                        perf.get_id().to_string(),
                                        perf.get_version().to_owned(),
                                        &all).await;

        Ok(())
    }

    async fn get_obj(&self, obj_id: ObjectId, _target: Option<ObjectId>) -> BuckyResult<NONGetObjectResponse> {
        let get_req = NONGetObjectRequest {
            common: NONOutputRequestCommon::new(NONAPILevel::Router),
            object_id: obj_id,
            inner_path: None
        };

        self.cyfs_stack.non_service().get_object(get_req).await
    }

    // NON_REQUEST_FLAG_SIGN_BY_DEVICE | NON_REQUEST_FLAG_SIGN_SET_DESC | NON_REQUEST_FLAG_SIGN_SET_BODY
    pub async fn put_object<D, T, N>(&self, obj: &N, target: Option<ObjectId>, sign_flags: u32) -> BuckyResult<()>
        where
            D: ObjectType,
            T: RawEncode,
            N: RawConvertTo<T>,
            N: NamedObject<D>, <D as ObjectType>::ContentType: BodyContent
    {
        // 给Obj签名, 用自己的Device
        let object_raw = obj.to_vec().unwrap();
        let object_id = obj.desc().calculate_id();
        let req = CryptoSignObjectRequest::new(object_id.clone(), object_raw, sign_flags);
        let resp = self.cyfs_stack.crypto().sign_object(req).await.map_err(|e| {
            error!("{} sign failed, err {}", &object_id, e);
            e
        })?;

        // 真正的put对象到目标
        let put_req = NONPutObjectRequest {
            common: NONOutputRequestCommon {
                req_path: None,
                dec_id: obj.desc().dec_id().clone(),
                level: NONAPILevel::Router,
                target,
                flags: 0
            },
            object: NONObjectInfo::new_from_object_raw(resp.object.unwrap().object_raw).unwrap()
        };
        match self.cyfs_stack.non_service().put_object(put_req).await {
            Ok(_) => {
                info!("### $$$ put obj [{}] to {} success!", object_id, target.map_or("ood".to_owned(), |id| id.to_string()));
                Ok(())
            }
            Err(e) => {
                error!("### $$$ put obj [{}] to {} failed! {}", object_id, target.map_or("ood".to_owned(), |id| id.to_string()), e);
                Err(e)
            }
        }
    }
}

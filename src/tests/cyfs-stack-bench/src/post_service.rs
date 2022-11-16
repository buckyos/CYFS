use cyfs_base::*;
use cyfs_core::{Text, TextObj};
use log::*;
use cyfs_lib::*;
use std::sync::Arc;
use std::str::FromStr;
use cyfs_util::EventListenerAsyncRoutine;
use crate::DEVICE_DEC_ID;
use crate::bench::GLOABL_STATE_PATH;
use crate::util::new_object;

pub const TEST_DEC_ID_STR: &str = "5aSixgP8EPf6HkP54Qgybddhhsd1fgrkg7Atf2icJiiS";
pub const CALL_PATH: &str = "/cyfs-bench-post";
pub const NON_CALL_PATH: &str = "/cyfs-bench-non";
pub const ROOT_STATE_CALL_PATH: &str = "/cyfs-bench-root-state";
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

enum ServiceType {
    TestPost,
    CrossZoneNonTest,
    CrossZoneRootStateTest,
}

struct OnPostObject {
    owner: Arc<TestService>,
    service_type: ServiceType,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for OnPostObject
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        let object = Text::clone_from_slice(&param.request.object.object_raw)?;
        match self.service_type {
            ServiceType::TestPost => {
                let answer = new_object("answer", object.header());
                let response = NONPostObjectInputResponse {
                    object: Some(NONObjectInfo::new(
                        answer.desc().calculate_id(),
                        answer.to_vec().unwrap(),
                        None,
                    )),
                };

                // 使用answer对象应答
                Ok(RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Response,
                    request: None,
                    response: Some(Ok(response)),
                })
            }
            ServiceType::CrossZoneNonTest => {
                if object.id() == "add" {
                    let value = object.header().parse::<usize>()?;
                    info!("generating test objects...");
                    let mut ids = Vec::with_capacity(value);
                    for i in 0..value {
                        let obj = new_object("obj", &i.to_string());
                        let mut req = NONPutObjectRequest::new_noc(obj.desc().calculate_id(), obj.to_vec().unwrap());
                        req.access = Some(AccessString::full());
                        self.owner.cyfs_stack.non_service().put_object(
                            req
                        ).await?;
                        ids.push(obj.desc().calculate_id());
                    }

                    let mut answer = new_object("add", "finish");
                    *answer.body_mut_expect("").content_mut().value_mut() = ids.to_hex().unwrap();
                    let response = NONPostObjectInputResponse {
                        object: Some(NONObjectInfo::new(
                            answer.desc().calculate_id(),
                            answer.to_vec().unwrap(),
                            None,
                        )),
                    };

                    Ok(RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Response,
                        request: None,
                        response: Some(Ok(response)),
                    })
                } else if object.id() == "remove" {
                    info!("delete test objects...");
                    let ids = Vec::<ObjectId>::clone_from_hex(object.value(), &mut vec![]).unwrap();
                    for id in ids {
                        self.owner.cyfs_stack.non_service().delete_object(
                            NONDeleteObjectRequest::new_noc(id.clone(), None)
                        ).await?;
                    }

                    let answer = new_object("remove", "finish");
                    let response = NONPostObjectInputResponse {
                        object: Some(NONObjectInfo::new(
                            answer.desc().calculate_id(),
                            answer.to_vec().unwrap(),
                            None,
                        )),
                    };

                    Ok(RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Response,
                        request: None,
                        response: Some(Ok(response)),
                    })
                } else {
                    Ok(RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Response,
                        request: None,
                        response: Some(Err(BuckyError::from(BuckyErrorCode::NotSupport))),
                    })
                }
            }

            ServiceType::CrossZoneRootStateTest => {
                if object.id() == "add" {
                    let value = object.header().parse::<usize>()?;
                    info!("generating test objects...");

                    let root_state = self.owner.cyfs_stack.root_state_stub(None, None);
                    let root_info = root_state.get_current_root().await.unwrap();
                    debug!("current root: {:?}", root_info);
                    let access = RootStateOpEnvAccess::new(GLOABL_STATE_PATH, AccessPermissions::ReadAndWrite);   // 对跨dec路径操作这个perm才work
                    let op_env = root_state.create_path_op_env_with_access(Some(access)).await.unwrap();
                    
                    let ret = op_env.get_by_path("/global-states/x/b").await.unwrap();
                    assert_eq!(ret, None);
                    let ret = op_env.get_by_path("/global-states/x/b/c").await.unwrap();
                    assert_eq!(ret, None);

                    for i in 0..value {
                        let obj = new_object("obj", &i.to_string());
                        op_env
                            .insert_with_key("/global-states/x/b", obj.desc().calculate_id().to_string(), &obj.desc().calculate_id())
                            .await
                            .unwrap();
                    }

                    let answer = new_object("add", "finish");
                    let response = NONPostObjectInputResponse {
                        object: Some(NONObjectInfo::new(
                            answer.desc().calculate_id(),
                            answer.to_vec().unwrap(),
                            None,
                        )),
                    };

                    Ok(RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Response,
                        request: None,
                        response: Some(Ok(response)),
                    })
                } else if object.id() == "remove" {
                    info!("delete test objects...");
                    let root_state = self.owner.cyfs_stack.root_state_stub(None, None);
                    let root_info = root_state.get_current_root().await.unwrap();
                    debug!("current root: {:?}", root_info);
                    let access = RootStateOpEnvAccess::new(GLOABL_STATE_PATH, AccessPermissions::ReadAndWrite);   // 对跨dec路径操作这个perm才work
                    let op_env = root_state.create_path_op_env_with_access(Some(access)).await.unwrap();

                    op_env.remove_with_path("/global-states/x/b", None).await.unwrap();

                    let answer = new_object("remove", "finish");
                    let response = NONPostObjectInputResponse {
                        object: Some(NONObjectInfo::new(
                            answer.desc().calculate_id(),
                            answer.to_vec().unwrap(),
                            None,
                        )),
                    };

                    Ok(RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Response,
                        request: None,
                        response: Some(Ok(response)),
                    })
                } else {
                    Ok(RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Response,
                        request: None,
                        response: Some(Err(BuckyError::from(BuckyErrorCode::NotSupport))),
                    })
                }
            }
        }

    }
}


pub struct TestService {
    pub(crate) device_info: DeviceInfo,
    cyfs_stack: SharedCyfsStack,
}

impl TestService {
    pub fn new(cyfs_stack: SharedCyfsStack) -> Self {
        let device_id = cyfs_stack.local_device_id().clone();
        let owner_id = PeopleId::default();

        let dec_id = ObjectId::from_str(TEST_DEC_ID_STR).unwrap();

        let device_info = DeviceInfo::new(owner_id, device_id, dec_id);

        info!("device {}, owner {}, dec {}", &device_info.ood_id, &device_info.owner_id, &device_info.dec_id);
        Self {
            cyfs_stack: cyfs_stack.clone(),
            device_info,
        }
    }

    pub async fn start(self) {
        let stub = self.cyfs_stack.root_state_meta_stub(None, None);
        stub.add_access(GlobalStatePathAccessItem::new_group(NON_CALL_PATH, None, None, Some(DEVICE_DEC_ID.clone()), AccessPermissions::CallOnly as u8)).await.unwrap();
        stub.add_access(GlobalStatePathAccessItem::new_group(CALL_PATH, None, None, Some(DEVICE_DEC_ID.clone()), AccessPermissions::CallOnly as u8)).await.unwrap();
        stub.add_access(GlobalStatePathAccessItem::new_group(ROOT_STATE_CALL_PATH, None, None, Some(DEVICE_DEC_ID.clone()), AccessPermissions::Full as u8)).await.unwrap();
        stub.add_access(GlobalStatePathAccessItem::new_group(CYFS_CRYPTO_VIRTUAL_PATH, None, None, Some(DEVICE_DEC_ID.clone()), AccessPermissions::CallOnly as u8)).await.unwrap();

        let service = Arc::new(self);

        // 只监听应用自己的DecObject
        service.cyfs_stack
            .router_handlers()
            .add_handler(
                RouterHandlerChain::Handler,
                "cyfs-bench-service",
                0,
                None,
                Some(CALL_PATH.to_owned()),
                RouterHandlerAction::Default,
                Some(Box::new(OnPostObject {
                    owner: service.clone(),
                    service_type: ServiceType::TestPost
                })))
            .unwrap();
        // 再加一个功能用的handler

        service.cyfs_stack
            .router_handlers()
            .add_handler(
                RouterHandlerChain::Handler,
                "cyfs-bench-non",
                0,
                None,
                Some(NON_CALL_PATH.to_owned()),
                RouterHandlerAction::Default,
                Some(Box::new(OnPostObject {
                    owner: service.clone(),
                    service_type: ServiceType::CrossZoneNonTest
                })))
            .unwrap();

        service.cyfs_stack
            .router_handlers()
            .add_handler(
                RouterHandlerChain::Handler,
                "cyfs-bench-root-state",
                0,
                None,
                Some(ROOT_STATE_CALL_PATH.to_owned()),
                RouterHandlerAction::Default,
                Some(Box::new(OnPostObject {
                    owner: service.clone(),
                    service_type: ServiceType::CrossZoneRootStateTest
                })))
            .unwrap();
    }
}
use cyfs_base::*;
use cyfs_core::{Text, TextObj};
use log::*;
use cyfs_lib::*;
use std::sync::Arc;
use std::str::FromStr;
use cyfs_util::EventListenerAsyncRoutine;
use crate::util::new_object;

pub const TEST_DEC_ID_STR: &str = "5aSixgP8EPf6HkP54Qgybddhhsd1fgrkg7Atf2icJiiS";
pub const CALL_PATH: &str = "/cyfs-bench-post";
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

struct OnPostObject {
    owner: Arc<TestService>,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for OnPostObject
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        info!("handler_post_object: {}", param.request.object.object_id);

        let object = Text::clone_from_slice(&param.request.object.object_raw)?;
        let answer = new_object("answer", object.value());
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

    pub fn start(self) {
        let service = Arc::new(self);
        // 注册on_post_put_router事件
        let listener = OnPostObject {
            owner: service.clone(),
        };

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
                Some(Box::new(listener)))
            .unwrap();

    }
}
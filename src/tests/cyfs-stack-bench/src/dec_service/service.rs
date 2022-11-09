use cyfs_base::*;
use cyfs_core::{Text, TextObj};
use log::*;
use cyfs_lib::*;
use std::sync::Arc;
use std::str::FromStr;
use cyfs_util::EventListenerAsyncRoutine;

pub const TEST_DEC_ID_STR: &str = "5aSixgP8EPf6HkP54Qgybddhhsd1fgrkg7Atf2icJiiS";

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

fn qa_pair() -> (Text, Text) {
    let q = Text::build("question", "test_header", "hello!")
        .no_create_time()
        .build();
    let a = Text::build("answer", "test_header", "world!")
        .no_create_time()
        .build();

    (q, a)
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

        let (q, a) = qa_pair();

        let object = Text::clone_from_slice(&param.request.object.object_raw).unwrap();
        let result = if *object.text_id().object_id() == *q.text_id().object_id() {
            let response = NONPostObjectInputResponse {
                object: Some(NONObjectInfo::new(
                    a.text_id().object_id().to_owned(),
                    a.to_vec().unwrap(),
                    None,
                )),
            };

            // 使用answer对象应答
            RouterHandlerPostObjectResult {
                action: RouterHandlerAction::Response,
                request: None,
                response: Some(Ok(response)),
            }
        } else {
            let msg = format!(
                "post object id not support! req={}",
                param.request.object.object_id
            );
            warn!("{}", msg);
            let response = Err(BuckyError::new(BuckyErrorCode::NotFound, msg));

            // 其余对象，直接返回
            RouterHandlerPostObjectResult {
                action: RouterHandlerAction::Response,
                request: None,
                response: Some(response),
            }
        };

        Ok(result)
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

    pub async fn init(&mut self) {

    }

    pub fn start(service: Arc<TestService>) {
        // 注册on_post_put_router事件
        let listener = OnPostObject {
            owner: service.clone(),
        };

        let dec_id = ObjectId::from_str(TEST_DEC_ID_STR).unwrap();
        let call_path = "/test_post";
        let req_path = RequestGlobalStatePath::new(Some(dec_id.clone()), Some(call_path.to_owned()));
        // 只监听应用自己的DecObject
        service.cyfs_stack
            .router_handlers()
            .add_handler(
                RouterHandlerChain::Handler,
                "must_same_source_target_dec",
                0,
                None,
                Some(req_path.to_string()),
                RouterHandlerAction::Default,
                Some(Box::new(listener)))
            .unwrap();

    }
}
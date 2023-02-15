use cyfs_base::*;
use cyfs_perf_base::*;
use log::*;
use cyfs_lib::*;
use async_trait::async_trait;
use std::sync::{Arc};
use cyfs_util::EventListenerAsyncRoutine;
use crate::config::{get_stack, PerfConfig};

use crate::storage::{create_storage, StorageRef};

struct OnPerfReport {
    owner: Arc<PerfService>,
}

#[async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult> for OnPerfReport {
    async fn call(&self, param: &RouterHandlerPostObjectRequest) -> BuckyResult<RouterHandlerPostObjectResult> {
        let mut result = RouterHandlerPostObjectResult {
            action: RouterHandlerAction::Response,
            request: None,
            response: None,
        };

        let owner = self.owner.clone();
        let from = param.request.common.source.zone.device.as_ref()
            .map(|o|o.to_string())
            .unwrap_or_else(|| {
                "self".to_owned()
            });

        // 验证对象签名，决定是否保存
        // 解出Perf对象
        match Perf::clone_from_slice(&param.request.object.object_raw) {
            Ok(perf) => {
                let id= perf.desc().calculate_id();
                if self.owner.verify_object(&perf).await {
                    // 这里是实际处理流程，不占用路由时间
                    async_std::task::spawn(async move {
                        info!("process perf object {} from {}", &id, from);
                        let _ = owner.on_perf(&perf).await;
                    });
                    result.response = Some(Ok(NONPostObjectInputResponse { object: None }))
                } else {
                    let msg = format!("perf object {} verify failed", &id);
                    warn!("{}", &msg);
                    result.response = Some(Err(BuckyError::new(BuckyErrorCode::Reject, msg)));
                };
            }
            Err(e) => {
                let msg = format!("decode perf object {} err {}", &param.request.object.object_id, e);
                result.response = Some(Err(BuckyError::new(BuckyErrorCode::InvalidInput, msg)));
            }
        }

        Ok(result)
    }
}

pub(crate) struct PerfService {
    cyfs_stack: SharedCyfsStack,
    perf_storage: StorageRef
}

impl PerfService {
    pub async fn create(config: PerfConfig) -> BuckyResult<Self> {
        let storage = create_storage(&config.storage).await?;
        Ok(Self {
            cyfs_stack: get_stack(config.stack_type).await?,
            perf_storage: storage
        })
    }

    pub fn start(service: Arc<PerfService>) {
        // 注册on_post_put_router事件
        let listener = OnPerfReport {
            owner: service.clone(),
        };

        // 只监听应用自己的DecObject
        service.cyfs_stack
            .router_handlers()
            .add_handler(
                RouterHandlerChain::Handler,
                "cyfs_perf_on_perf_report",
                0,
                None,
                Some(PERF_REPORT_PATH.to_owned()),
                RouterHandlerAction::Default,
                Some(Box::new(listener)))
            .unwrap();
    }

    // 这里验证对象签名是否正确，验证正确的对象才会被保存
    pub async fn verify_object(&self, _pref: &Perf) -> bool {
        // 这里用业务逻辑检查
        true
    }

    async fn on_perf(&self, perf: &Perf) -> BuckyResult<()> {
        let id = perf.desc().calculate_id();

        info!(
            "###### recv msg {}, people:{}, device:{}, dec_id: {}, id: {}",
            &id,
            perf.people(),
            perf.device(),
            perf.dec_id(),
            perf.get_id()
        );

        let all = perf.get_entity_list();
        info!("perf entity list len: {}", all.len());

        let _ = self.perf_storage.insert_entity_list(perf.people(),
                                        perf.device(),
                                        perf.dec_id().to_string(),
                                        perf.get_id().to_string(),
                                        perf.get_version().to_owned(),
                                        &all).await;

        Ok(())
    }
}

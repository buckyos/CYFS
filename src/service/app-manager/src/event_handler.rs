use crate::app_manager_ex::AppManager;
use async_trait::async_trait;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;
use log::*;
use std::sync::Arc;

pub struct EventListener {
    pub app_manager: Arc<AppManager>,
}

#[async_trait]
impl EventListenerAsyncRoutine<RouterHandlerPostObjectRequest, RouterHandlerPostObjectResult>
    for EventListener
{
    async fn call(
        &self,
        param: &RouterHandlerPostObjectRequest,
    ) -> BuckyResult<RouterHandlerPostObjectResult> {
        if CoreObjectType::from(param.request.object.object.as_ref().unwrap().obj_type())
            != CoreObjectType::AppCmd
        {
            return Ok(RouterHandlerPostObjectResult {
                action: RouterHandlerAction::Pass,
                request: None,
                response: None,
            });
        }

        match AppCmd::clone_from_slice(&param.request.object.object_raw) {
            Ok(cmd) => match self.app_manager.on_app_cmd(cmd, true).await {
                Ok(_) => {
                    let resp = NONPostObjectInputResponse { object: None };

                    Ok(RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Response,
                        request: None,
                        response: Some(Ok(resp)),
                    })
                }
                Err(e) => {
                    error!("process app_cmd error {:?}", e);
                    Ok(RouterHandlerPostObjectResult {
                        action: RouterHandlerAction::Response,
                        request: None,
                        response: Some(Err(e)),
                    })
                }
            },
            Err(e) => {
                warn!("get cmd object failed. req:{}", param.request);
                Ok(RouterHandlerPostObjectResult {
                    action: RouterHandlerAction::Response,
                    request: None,
                    response: Some(Err(e)),
                })
            }
        }
    }
}

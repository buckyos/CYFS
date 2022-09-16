use super::helper::*;
use crate::router_handler::RouterHandlerEmitter;
use cyfs_base::*;
use cyfs_lib::*;

use std::{fmt, str::FromStr};

pub(crate) struct NONHandlerCaller<REQ, RESP>
where
    REQ: Send
        + Sync
        + 'static
        + ExpReservedTokenTranslator
        + JsonCodec<REQ>
        + fmt::Display
        + RequestHandlerHelper<REQ>,
    RESP: Send
        + Sync
        + 'static
        + ExpReservedTokenTranslator
        + JsonCodec<RESP>
        + fmt::Display
        + RequestHandlerHelper<RESP>,
    RouterHandlerRequest<REQ, RESP>: ExpReservedTokenTranslator + RouterHandlerCategoryInfo,
{
    emitter: RouterHandlerEmitter<REQ, RESP>,
}

impl<REQ, RESP> NONHandlerCaller<REQ, RESP>
where
    REQ: Send
        + Sync
        + 'static
        + ExpReservedTokenTranslator
        + JsonCodec<REQ>
        + fmt::Display
        + RequestHandlerHelper<REQ>,
    RESP: Send
        + Sync
        + 'static
        + ExpReservedTokenTranslator
        + JsonCodec<RESP>
        + fmt::Display
        + RequestHandlerHelper<RESP>,
    RouterHandlerRequest<REQ, RESP>: ExpReservedTokenTranslator + RouterHandlerCategoryInfo,
{
    pub fn new(emitter: RouterHandlerEmitter<REQ, RESP>) -> Self {
        Self { emitter }
    }

    pub async fn call(
        &mut self,
        name: &str,
        param: &mut RouterHandlerRequest<REQ, RESP>,
    ) -> BuckyResult<Option<BuckyResult<RESP>>> {
        let default_action = RouterHandlerAction::Default;

        let req_path = if let Some(req_path) = param.request.req_path() {
            Some(RequestGlobalStatePath::from_str(&req_path)?)
        } else {
            None
        };

        loop {
            // 最终会返回非pass的default_action，结束循环
            let resp = self.emitter.next(&req_path, &param, &default_action).await;
            info!(
                "non {} handler resp: chain={}, category={}, req={}, {}",
                name,
                self.emitter.chain(),
                self.emitter.category(),
                param.request.debug_info(),
                resp
            );

            let result = match resp.action {
                RouterHandlerAction::Pass => {
                    // 如果返回了request，那么尝试更新
                    if let Some(new_req) = resp.request {
                        param.request.update(new_req);
                    }

                    // 如果返回了response，那么也尝试更新
                    if let Some(new_resp) = resp.response {
                        if let Some(res) = param.response.as_mut() {
                            res.update(new_resp);
                        } else {
                            param.response = Some(new_resp);
                        }
                    }
                    continue;
                }

                RouterHandlerAction::Default => {
                    // 如果返回了request，那么尝试更新
                    if let Some(new_req) = resp.request {
                        param.request.update(new_req);
                    }

                    // 如果返回了response，那么也尝试更新
                    if let Some(new_resp) = resp.response {
                        if let Some(res) = param.response.as_mut() {
                            res.update(new_resp);
                        } else {
                            param.response = Some(new_resp);
                        }
                    }

                    Ok(None)
                }
                RouterHandlerAction::Response => {
                    if let Some(resp) = resp.response {
                        Ok(Some(resp))
                    } else {
                        let msg = format!(
                            "non {} handler return resp action but empty response! chain={}, category={}, req={}",
                            name,
                            self.emitter.chain(),
                            self.emitter.category(),
                            param.request.debug_info(),
                        );
                        warn!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::InternalError, msg))
                    }
                }
                RouterHandlerAction::Reject => {
                    let msg = format!(
                        "non {} rejected default or by handler! chain={}, category={}, req={}",
                        name,
                        self.emitter.chain(),
                        self.emitter.category(),
                        param.request.debug_info(),
                    );
                    warn!("{}", msg);

                    Err(BuckyError::new(BuckyErrorCode::Reject, msg))
                }
                RouterHandlerAction::Drop => {
                    let msg = format!(
                        "non {} dropped default or by handler! chain={}, category={}, req={}",
                        name,
                        self.emitter.chain(),
                        self.emitter.category(),
                        param.request.debug_info(),
                    );
                    warn!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::Ignored, msg))
                }
            };

            break result;
        }
    }
}

use super::super::{RouterHandler, RouterHandlersManager};
use super::ws_routine::RouterHandlerWebSocketRoutine;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;

use std::fmt;
use std::sync::Arc;

#[derive(Clone)]
pub(crate) struct RouterHandlerWSProcessor {
    manager: RouterHandlersManager,
}

impl RouterHandlerWSProcessor {
    pub fn new(manager: RouterHandlersManager) -> Self {
        Self { manager }
    }

    fn create_handler<REQ, RESP>(
        session_requestor: Arc<WebSocketRequestManager>,
        source: &RequestSourceInfo,
        req: &RouterWSAddHandlerParam,
    ) -> BuckyResult<RouterHandler<REQ, RESP>>
    where
        REQ: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + fmt::Display,
        RESP: Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<RESP> + fmt::Display,
        RouterHandlerRequest<REQ, RESP>: RouterHandlerCategoryInfo,
    {
        info!(
            "new router ws handler: sid={}, chain={}, category={}, id={}, filter={:?}, req_path={:?}, default_action={}, routine={:?}",
            session_requestor.sid(), req.chain, req.category, req.id, req.param.filter, req.param.req_path, req.param.default_action, req.param.routine
        );

        let routine = if req.param.routine.is_some() {
            let r = RouterHandlerWebSocketRoutine::<REQ, RESP>::new(
                &req.chain,
                &req.category,
                &req.id,
                session_requestor.clone(),
            )?;
            Some(Box::new(r)
                as Box<
                    dyn EventListenerAsyncRoutine<
                        RouterHandlerRequest<REQ, RESP>,
                        RouterHandlerResponse<REQ, RESP>,
                    >,
                >)
        } else {
            None
        };

        let handler = RouterHandler::new(
            source,
            req.id.clone(),
            req.dec_id,
            req.param.index,
            req.param.filter.clone(),
            req.param.req_path.clone(),
            req.param.default_action.clone(),
            routine,
        )?;

        Ok(handler)
    }

    pub async fn on_add_handler_request(
        &self,
        session_requestor: Arc<WebSocketRequestManager>,
        source: RequestSourceInfo,
        req: &RouterWSAddHandlerParam,
    ) -> BuckyResult<()> {

        // check access
        self.manager
            .check_access(
                &source,
                req.chain,
                req.category,
                &req.id,
                &req.param.req_path,
                &req.param.filter,
            )
            .await?;

        match req.category {
            RouterHandlerCategory::PutObject => {
                let handler = Self::create_handler::<
                    NONPutObjectInputRequest,
                    NONPutObjectInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .put_object()
                    .add_handler(handler)
            }

            RouterHandlerCategory::GetObject => {
                let handler = Self::create_handler::<
                    NONGetObjectInputRequest,
                    NONGetObjectInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .get_object()
                    .add_handler(handler)
            }

            RouterHandlerCategory::PostObject => {
                let handler = Self::create_handler::<
                    NONPostObjectInputRequest,
                    NONPostObjectInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .post_object()
                    .add_handler(handler)
            }

            RouterHandlerCategory::SelectObject => {
                let handler = Self::create_handler::<
                    NONSelectObjectInputRequest,
                    NONSelectObjectInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .select_object()
                    .add_handler(handler)
            }
            RouterHandlerCategory::DeleteObject => {
                let handler = Self::create_handler::<
                    NONDeleteObjectInputRequest,
                    NONDeleteObjectInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .delete_object()
                    .add_handler(handler)
            }

            RouterHandlerCategory::GetData => {
                let handler = Self::create_handler::<
                    NDNGetDataInputRequest,
                    NDNGetDataInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .get_data()
                    .add_handler(handler)
            }
            RouterHandlerCategory::PutData => {
                let handler = Self::create_handler::<
                    NDNPutDataInputRequest,
                    NDNPutDataInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .put_data()
                    .add_handler(handler)
            }
            RouterHandlerCategory::DeleteData => {
                let handler = Self::create_handler::<
                    NDNDeleteDataInputRequest,
                    NDNDeleteDataInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .delete_data()
                    .add_handler(handler)
            }

            RouterHandlerCategory::SignObject => {
                let handler = Self::create_handler::<
                    CryptoSignObjectInputRequest,
                    CryptoSignObjectInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .sign_object()
                    .add_handler(handler)
            }
            RouterHandlerCategory::VerifyObject => {
                let handler = Self::create_handler::<
                    CryptoVerifyObjectInputRequest,
                    CryptoVerifyObjectInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .verify_object()
                    .add_handler(handler)
            }

            RouterHandlerCategory::EncryptData => {
                let handler = Self::create_handler::<
                    CryptoEncryptDataInputRequest,
                    CryptoEncryptDataInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .encrypt_data()
                    .add_handler(handler)
            }
            RouterHandlerCategory::DecryptData => {
                let handler = Self::create_handler::<
                    CryptoDecryptDataInputRequest,
                    CryptoDecryptDataInputResponse,
               >(session_requestor, &source, &req)?;
                self.manager
                    .handlers(&req.chain)
                    .decrypt_data()
                    .add_handler(handler)
            }

            RouterHandlerCategory::Acl => {
                let handler = Self::create_handler::<AclHandlerRequest, AclHandlerResponse>(
                    session_requestor,
                    &source,
                    &req,
                )?;
                self.manager.handlers(&req.chain).acl().add_handler(handler)
            }, 

            RouterHandlerCategory::Interest => {
                let handler = Self::create_handler::<InterestHandlerRequest, InterestHandlerResponse>(
                    session_requestor,
                    &source,
                    &req,
                )?;
                self.manager.handlers(&req.chain).interest().add_handler(handler)
            }
        }
    }

    pub fn on_remove_handler_request(&self, req: RouterWSRemoveHandlerParam) -> BuckyResult<bool> {
        let ret = match req.category {
            RouterHandlerCategory::PutObject => self
                .manager
                .handlers(&req.chain)
                .put_object()
                .remove_handler(&req.id, req.dec_id),

            RouterHandlerCategory::GetObject => self
                .manager
                .handlers(&req.chain)
                .get_object()
                .remove_handler(&req.id, req.dec_id),

            RouterHandlerCategory::PostObject => self
                .manager
                .handlers(&req.chain)
                .post_object()
                .remove_handler(&req.id, req.dec_id),

            RouterHandlerCategory::SelectObject => self
                .manager
                .handlers(&req.chain)
                .select_object()
                .remove_handler(&req.id, req.dec_id),

            RouterHandlerCategory::DeleteObject => self
                .manager
                .handlers(&req.chain)
                .delete_object()
                .remove_handler(&req.id, req.dec_id),

            RouterHandlerCategory::GetData => self
                .manager
                .handlers(&req.chain)
                .get_data()
                .remove_handler(&req.id, req.dec_id),
            RouterHandlerCategory::PutData => self
                .manager
                .handlers(&req.chain)
                .put_data()
                .remove_handler(&req.id, req.dec_id),
            RouterHandlerCategory::DeleteData => self
                .manager
                .handlers(&req.chain)
                .delete_data()
                .remove_handler(&req.id, req.dec_id),

            RouterHandlerCategory::SignObject => self
                .manager
                .handlers(&req.chain)
                .sign_object()
                .remove_handler(&req.id, req.dec_id),
            RouterHandlerCategory::VerifyObject => self
                .manager
                .handlers(&req.chain)
                .verify_object()
                .remove_handler(&req.id, req.dec_id),
            RouterHandlerCategory::EncryptData => self
                .manager
                .handlers(&req.chain)
                .encrypt_data()
                .remove_handler(&req.id, req.dec_id),
            RouterHandlerCategory::DecryptData => self
                .manager
                .handlers(&req.chain)
                .decrypt_data()
                .remove_handler(&req.id, req.dec_id),

            RouterHandlerCategory::Acl => self
                .manager
                .handlers(&req.chain)
                .acl()
                .remove_handler(&req.id, req.dec_id),

            RouterHandlerCategory::Interest => self
                .manager
                .handlers(&req.chain)
                .interest()
                .remove_handler(&req.id, req.dec_id),
        };

        Ok(ret)
    }
}

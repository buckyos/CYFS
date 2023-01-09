use super::super::{RouterHandler, RouterHandlersManager};
use super::http_routine::RouterHandlerHttpRoutine;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;

pub(crate) struct RouterAddHandlerRequest {
    pub param: RouterAddHandlerParam,

    pub chain: RouterHandlerChain,
    pub category: RouterHandlerCategory,

    pub id: String,

    pub protocol: RequestProtocol,

    // source device
    pub source: Option<DeviceId>,

    // source dec_id
    pub dec_id: Option<ObjectId>,
}

pub(crate) struct RouterRemoveHandlerRequest {
    pub chain: RouterHandlerChain,
    pub category: RouterHandlerCategory,

    pub id: String,

    pub protocol: RequestProtocol,

    // source device
    pub source: Option<DeviceId>,

    // source dec_id
    pub dec_id: Option<ObjectId>,
}

#[derive(Clone)]
pub(crate) struct RouterHandlerHttpProcessor {
    manager: RouterHandlersManager,
}

impl RouterHandlerHttpProcessor {
    pub fn new(manager: RouterHandlersManager) -> Self {
        Self { manager }
    }

    fn create_handler<REQ, RESP>(
        source: &RequestSourceInfo,
        req: RouterAddHandlerRequest,
    ) -> BuckyResult<RouterHandler<REQ, RESP>>
    where
        REQ:
            Send + Sync + 'static + ExpReservedTokenTranslator + JsonCodec<REQ> + std::fmt::Display,
        RESP: Send
            + Sync
            + 'static
            + ExpReservedTokenTranslator
            + JsonCodec<RESP>
            + std::fmt::Display,
        RouterHandlerRequest<REQ, RESP>: RouterHandlerCategoryInfo,
    {
        let routine = if req.param.routine.is_some() {
            let r = RouterHandlerHttpRoutine::<REQ, RESP>::new(
                &req.chain,
                &req.category,
                &req.id,
                req.param.routine.as_ref().unwrap(),
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

        info!(
            "new router handler: category: {}, id: {}, dec: {:?} filter: {:?}, req_path: {:?} default action: {}, routine: {:?}",
            req.category.to_string(), req.id, req.dec_id, req.param.filter, req.param.req_path, req.param.default_action, req.param.routine
        );

        let handler = RouterHandler::new(
            source,
            req.id,
            req.dec_id,
            req.param.index,
            req.param.filter,
            req.param.req_path,
            req.param.default_action,
            routine,
        )?;

        Ok(handler)
    }

    pub async fn on_add_handler_request(
        &self,
        source: RequestSourceInfo,
        req: RouterAddHandlerRequest,
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

        let chain = req.chain.clone();
        match req.category {
            RouterHandlerCategory::PutObject => {
                let handler = Self::create_handler::<
                    NONPutObjectInputRequest,
                    NONPutObjectInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .put_object()
                    .add_handler(handler)
            }
            RouterHandlerCategory::GetObject => {
                let handler = Self::create_handler::<
                    NONGetObjectInputRequest,
                    NONGetObjectInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .get_object()
                    .add_handler(handler)
            }
            RouterHandlerCategory::PostObject => {
                let handler = Self::create_handler::<
                    NONPostObjectInputRequest,
                    NONPostObjectInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .post_object()
                    .add_handler(handler)
            }
            RouterHandlerCategory::SelectObject => {
                let handler = Self::create_handler::<
                    NONSelectObjectInputRequest,
                    NONSelectObjectInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .select_object()
                    .add_handler(handler)
            }
            RouterHandlerCategory::DeleteObject => {
                let handler = Self::create_handler::<
                    NONDeleteObjectInputRequest,
                    NONDeleteObjectInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .delete_object()
                    .add_handler(handler)
            }

            RouterHandlerCategory::GetData => {
                let handler = Self::create_handler::<
                    NDNGetDataInputRequest,
                    NDNGetDataInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .get_data()
                    .add_handler(handler)
            }
            RouterHandlerCategory::PutData => {
                let handler = Self::create_handler::<
                    NDNPutDataInputRequest,
                    NDNPutDataInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .put_data()
                    .add_handler(handler)
            }
            RouterHandlerCategory::DeleteData => {
                let handler = Self::create_handler::<
                    NDNDeleteDataInputRequest,
                    NDNDeleteDataInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .delete_data()
                    .add_handler(handler)
            }

            RouterHandlerCategory::SignObject => {
                let handler = Self::create_handler::<
                    CryptoSignObjectInputRequest,
                    CryptoSignObjectInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .sign_object()
                    .add_handler(handler)
            }
            RouterHandlerCategory::VerifyObject => {
                let handler = Self::create_handler::<
                    CryptoVerifyObjectInputRequest,
                    CryptoVerifyObjectInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .verify_object()
                    .add_handler(handler)
            }

            RouterHandlerCategory::EncryptData => {
                let handler = Self::create_handler::<
                    CryptoEncryptDataInputRequest,
                    CryptoEncryptDataInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .encrypt_data()
                    .add_handler(handler)
            }
            RouterHandlerCategory::DecryptData => {
                let handler = Self::create_handler::<
                    CryptoDecryptDataInputRequest,
                    CryptoDecryptDataInputResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .decrypt_data()
                    .add_handler(handler)
            }

            RouterHandlerCategory::Acl => {
                let handler =
                    Self::create_handler::<AclHandlerRequest, AclHandlerResponse>(&source, req)?;
                self.manager.handlers(&chain).acl().add_handler(handler)
            }

            RouterHandlerCategory::Interest => {
                let handler = Self::create_handler::<
                    InterestHandlerRequest,
                    InterestHandlerResponse,
                >(&source, req)?;
                self.manager
                    .handlers(&chain)
                    .interest()
                    .add_handler(handler)
            }
        }
    }

    pub async fn on_remove_handler_request(
        &self,
        req: RouterRemoveHandlerRequest,
    ) -> BuckyResult<bool> {
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

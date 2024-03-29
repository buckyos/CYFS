use super::handler::RouterHandler;
use super::handler_manager::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_util::*;

pub struct SharedRouterHandlersManager {
    manager: RouterHandlersManager,
    source: RequestSourceInfo,
}

impl SharedRouterHandlersManager {
    pub fn new(manager: RouterHandlersManager, source: RequestSourceInfo) -> Self {
        Self {
            manager,
            source,
        }
    }
}

macro_rules! declare_router_handler_processor {
    ($REQ:ty, $RESP:ty, $func:ident) => {
        #[async_trait::async_trait]
        impl RouterHandlerProcessor<$REQ, $RESP> for SharedRouterHandlersManager {
            async fn add_handler(
                &self,
                chain: RouterHandlerChain,
                id: &str,
                index: i32,
                filter: Option<String>,
                req_path: Option<String>,
                default_action: RouterHandlerAction,
                routine: Option<
                    Box<
                        dyn EventListenerAsyncRoutine<
                            RouterHandlerRequest<$REQ, $RESP>,
                            RouterHandlerResponse<$REQ, $RESP>,
                        >,
                    >,
                >,
            ) -> BuckyResult<()> {
                let handler = RouterHandler::new(
                    &self.source,
                    id.to_owned(),
                    None,
                    index,
                    filter,
                    req_path,
                    default_action,
                    routine,
                )?;

                self.manager.handlers(&chain).$func().add_handler(handler)
            }

            async fn remove_handler(
                &self,
                chain: RouterHandlerChain,
                id: &str,
            ) -> BuckyResult<bool> {
                let ret = self.manager.handlers(&chain).$func().remove_handler(id, None);

                Ok(ret)
            }
        }
    };
}

// non handlers
declare_router_handler_processor!(
    NONGetObjectInputRequest,
    NONGetObjectInputResponse,
    get_object
);
declare_router_handler_processor!(
    NONPutObjectInputRequest,
    NONPutObjectInputResponse,
    put_object
);
declare_router_handler_processor!(
    NONPostObjectInputRequest,
    NONPostObjectInputResponse,
    post_object
);
declare_router_handler_processor!(
    NONSelectObjectInputRequest,
    NONSelectObjectInputResponse,
    select_object
);
declare_router_handler_processor!(
    NONDeleteObjectInputRequest,
    NONDeleteObjectInputResponse,
    delete_object
);

// ndn handlers
declare_router_handler_processor!(NDNGetDataInputRequest, NDNGetDataInputResponse, get_data);
declare_router_handler_processor!(NDNPutDataInputRequest, NDNPutDataInputResponse, put_data);
declare_router_handler_processor!(
    NDNDeleteDataInputRequest,
    NDNDeleteDataInputResponse,
    delete_data
);

// crypto handlers
declare_router_handler_processor!(
    CryptoSignObjectInputRequest,
    CryptoSignObjectInputResponse,
    sign_object
);
declare_router_handler_processor!(
    CryptoVerifyObjectInputRequest,
    CryptoVerifyObjectInputResponse,
    verify_object
);

declare_router_handler_processor!(
    CryptoEncryptDataInputRequest,
    CryptoEncryptDataInputResponse,
    encrypt_data
);
declare_router_handler_processor!(
    CryptoDecryptDataInputRequest,
    CryptoDecryptDataInputResponse,
    decrypt_data
);

// acl handlers
declare_router_handler_processor!(AclHandlerRequest, AclHandlerResponse, acl);

// interest handlers
declare_router_handler_processor!(InterestHandlerRequest, InterestHandlerResponse, interest);

impl RouterHandlerManagerProcessor for SharedRouterHandlersManager {
    fn get_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONGetObjectInputRequest, NONGetObjectInputResponse> {
        self
    }

    fn put_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONPutObjectInputRequest, NONPutObjectInputResponse> {
        self
    }

    fn post_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONPostObjectInputRequest, NONPostObjectInputResponse> {
        self
    }

    fn select_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONSelectObjectInputRequest, NONSelectObjectInputResponse>
    {
        self
    }

    fn delete_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONDeleteObjectInputRequest, NONDeleteObjectInputResponse>
    {
        self
    }

    fn get_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<NDNGetDataInputRequest, NDNGetDataInputResponse> {
        self
    }
    fn put_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<NDNPutDataInputRequest, NDNPutDataInputResponse> {
        self
    }
    fn delete_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<NDNDeleteDataInputRequest, NDNDeleteDataInputResponse> {
        self
    }

    fn sign_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<CryptoSignObjectInputRequest, CryptoSignObjectInputResponse>
    {
        self
    }
    fn verify_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<CryptoVerifyObjectInputRequest, CryptoVerifyObjectInputResponse>
    {
        self
    }
    fn encrypt_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<CryptoEncryptDataInputRequest, CryptoEncryptDataInputResponse> {
        self
    }

    fn decrypt_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<CryptoDecryptDataInputRequest, CryptoDecryptDataInputResponse> {
        self
    }


    fn acl(&self) -> &dyn RouterHandlerProcessor<AclHandlerRequest, AclHandlerResponse> {
        self
    }

    fn interest(&self) -> &dyn RouterHandlerProcessor<InterestHandlerRequest, InterestHandlerResponse> {
        self
    }
}

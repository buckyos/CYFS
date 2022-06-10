use super::handler::RouterHandler;
use super::handler_manager::*;
use cyfs_base::*;
use cyfs_util::*;
use cyfs_lib::*;

macro_rules! declare_router_handler_processor {
    ($REQ:ty, $RESP:ty, $func:ident) => {
        #[async_trait::async_trait]
        impl RouterHandlerProcessor<$REQ, $RESP> for RouterHandlersManager {
            async fn add_handler(
                &self,
                chain: RouterHandlerChain,
                id: &str,
                index: i32,
                filter: &str,
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
                let handler =
                    RouterHandler::new(id.to_owned(), None, index, filter, default_action, routine)?;

                self.handlers(&chain).$func().add_handler(handler)
            }

            async fn remove_handler(
                &self,
                chain: RouterHandlerChain,
                id: &str,
            ) -> BuckyResult<bool> {
                let ret = self.handlers(&chain).$func().remove_handler(id, None);

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

// acl handlers
declare_router_handler_processor!(AclHandlerRequest, AclHandlerResponse, acl);

impl RouterHandlerManagerProcessor for RouterHandlersManager {
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

    fn acl(&self) -> &dyn RouterHandlerProcessor<AclHandlerRequest, AclHandlerResponse> {
        self
    }
}

use super::action::*;
use super::category::*;
use super::chain::*;
use super::request::*;
use crate::acl::*;
use crate::crypto::*;
use crate::ndn::*;
use crate::non::*;
use cyfs_base::*;
use cyfs_util::*;

use std::sync::Arc;

#[async_trait::async_trait]
pub trait RouterHandlerProcessor<REQ, RESP>: Send + Sync
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + std::fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + std::fmt::Display,
    RouterHandlerRequest<REQ, RESP>: RouterHandlerCategoryInfo,
{
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
                    RouterHandlerRequest<REQ, RESP>,
                    RouterHandlerResponse<REQ, RESP>,
                >,
            >,
        >,
    ) -> BuckyResult<()>;

    async fn remove_handler(&self, chain: RouterHandlerChain, id: &str) -> BuckyResult<bool>;
}

pub trait RouterHandlerManagerProcessor: Send + Sync {
    fn get_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONGetObjectInputRequest, NONGetObjectInputResponse>;
    fn put_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONPutObjectInputRequest, NONPutObjectInputResponse>;
    fn post_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONPostObjectInputRequest, NONPostObjectInputResponse>;
    fn select_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONSelectObjectInputRequest, NONSelectObjectInputResponse>;
    fn delete_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<NONDeleteObjectInputRequest, NONDeleteObjectInputResponse>;

    fn get_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<NDNGetDataInputRequest, NDNGetDataInputResponse>;
    fn put_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<NDNPutDataInputRequest, NDNPutDataInputResponse>;
    fn delete_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<NDNDeleteDataInputRequest, NDNDeleteDataInputResponse>;

    fn sign_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<CryptoSignObjectInputRequest, CryptoSignObjectInputResponse>;
    fn verify_object(
        &self,
    ) -> &dyn RouterHandlerProcessor<CryptoVerifyObjectInputRequest, CryptoVerifyObjectInputResponse>;
    fn encrypt_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<CryptoEncryptDataInputRequest, CryptoEncryptDataInputResponse>;
    fn decrypt_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<CryptoDecryptDataInputRequest, CryptoDecryptDataInputResponse>;

    fn acl(&self) -> &dyn RouterHandlerProcessor<AclHandlerRequest, AclHandlerResponse>;

    fn interest(
        &self,
    ) -> &dyn RouterHandlerProcessor<InterestHandlerRequest, InterestHandlerResponse>;
}

pub type RouterHandlerManagerProcessorRef = Arc<Box<dyn RouterHandlerManagerProcessor>>;

use super::action::*;
use super::category::*;
use super::chain::*;
use super::request::*;
use super::ws::*;
use crate::acl::*;
use crate::crypto::*;
use crate::ndn::*;
use crate::non::*;
use crate::stack::*;
use cyfs_base::*;
use cyfs_util::*;

use async_trait::async_trait;
use http_types::Url;
use std::fmt;
use std::sync::Arc;

#[async_trait]
pub(crate) trait RouterHandlerAnyRoutine: Send + Sync {
    async fn emit(&self, param: String) -> BuckyResult<String>;
}

pub(crate) struct RouterHandlerRoutineT<REQ, RESP>(
    pub  Box<
        dyn EventListenerAsyncRoutine<
            RouterHandlerRequest<REQ, RESP>,
            RouterHandlerResponse<REQ, RESP>,
        >,
    >,
)
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display;

#[async_trait]
impl<REQ, RESP> RouterHandlerAnyRoutine for RouterHandlerRoutineT<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    async fn emit(&self, param: String) -> BuckyResult<String> {
        let param = RouterHandlerRequest::<REQ, RESP>::decode_string(&param)?;
        self.0
            .call(&param)
            .await
            .map(|resp| JsonCodec::encode_string(&resp))
    }
}

#[derive(Clone)]
pub struct RouterHandlerManager {
    dec_id: Option<SharedObjectStackDecID>,

    inner: RouterWSHandlerManager,
}

impl RouterHandlerManager {
    pub async fn new(dec_id: Option<SharedObjectStackDecID>, ws_url: Url) -> BuckyResult<Self> {
        let inner = RouterWSHandlerManager::new(ws_url);
        inner.start();

        Ok(Self {
            dec_id,
            inner,
        })
    }

    fn get_dec_id(&self) -> Option<ObjectId> {
        self.dec_id.as_ref().map(|v| v.get().cloned()).flatten()
    }

    pub fn clone_processor(&self) -> RouterHandlerManagerProcessorRef {
        Arc::new(Box::new(self.clone()))
    }

    pub fn add_handler<REQ, RESP>(
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
    ) -> BuckyResult<()>
    where
        REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
        RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
        RouterHandlerRequest<REQ, RESP>: RouterHandlerCategoryInfo,
    {
        info!("will add handler: chain={}, id={}, index={}, filter={:?}, req_path={:?}, default_action={}",
            chain, id, index, filter, req_path, default_action);

        self.inner.add_handler(
            chain,
            id,
            self.get_dec_id(),
            index,
            filter,
            req_path,
            default_action,
            routine,
        )
    }

    pub async fn remove_handler(
        &self,
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: &str,
    ) -> BuckyResult<bool> {
        info!(
            "will remove handler: chain={}, category={}, id={}",
            chain, id, category
        );

        self.inner
            .remove_handler(chain, category, id, self.get_dec_id())
            .await
    }

    pub async fn stop(&self) {
        self.inner.stop().await
    }
}

use super::processor::*;

#[async_trait::async_trait]
impl<REQ, RESP> RouterHandlerProcessor<REQ, RESP> for RouterHandlerManager
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
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
    ) -> BuckyResult<()> {
        Self::add_handler(
            &self,
            chain,
            id,
            index,
            filter,
            req_path,
            default_action,
            routine,
        )
    }

    async fn remove_handler(&self, chain: RouterHandlerChain, id: &str) -> BuckyResult<bool> {
        let category = extract_router_handler_category::<RouterHandlerRequest<REQ, RESP>>();
        Self::remove_handler(&self, chain, category, id).await
    }
}

impl RouterHandlerManagerProcessor for RouterHandlerManager {
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
    ) -> &dyn RouterHandlerProcessor<CryptoEncryptDataInputRequest, CryptoEncryptDataInputResponse>
    {
        self
    }
    fn decrypt_data(
        &self,
    ) -> &dyn RouterHandlerProcessor<CryptoDecryptDataInputRequest, CryptoDecryptDataInputResponse>
    {
        self
    }

    fn acl(&self) -> &dyn RouterHandlerProcessor<AclHandlerRequest, AclHandlerResponse> {
        self
    }

    fn interest(
        &self,
    ) -> &dyn RouterHandlerProcessor<InterestHandlerRequest, InterestHandlerResponse> {
        self
    }
}

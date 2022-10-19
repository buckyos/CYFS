use super::handler::*;
use super::processor::SharedRouterHandlersManager;
use super::storage::*;
use crate::acl::AclManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

use once_cell::sync::OnceCell;
use std::sync::Arc;

pub struct RouterHandlersContainer {
    pub chain: RouterHandlerChain,
    pub storage: RouterHandlersStorage,

    pub put_object: OnceCell<RouterHandlers<NONPutObjectInputRequest, NONPutObjectInputResponse>>,
    pub get_object: OnceCell<RouterHandlers<NONGetObjectInputRequest, NONGetObjectInputResponse>>,
    pub post_object:
        OnceCell<RouterHandlers<NONPostObjectInputRequest, NONPostObjectInputResponse>>,

    pub select_object:
        OnceCell<RouterHandlers<NONSelectObjectInputRequest, NONSelectObjectInputResponse>>,
    pub delete_object:
        OnceCell<RouterHandlers<NONDeleteObjectInputRequest, NONDeleteObjectInputResponse>>,

    pub get_data: OnceCell<RouterHandlers<NDNGetDataInputRequest, NDNGetDataInputResponse>>,
    pub put_data: OnceCell<RouterHandlers<NDNPutDataInputRequest, NDNPutDataInputResponse>>,
    pub delete_data:
        OnceCell<RouterHandlers<NDNDeleteDataInputRequest, NDNDeleteDataInputResponse>>,

    pub sign_object:
        OnceCell<RouterHandlers<CryptoSignObjectInputRequest, CryptoSignObjectInputResponse>>,
    pub verify_object:
        OnceCell<RouterHandlers<CryptoVerifyObjectInputRequest, CryptoVerifyObjectInputResponse>>,

    pub encrypt_data:
        OnceCell<RouterHandlers<CryptoEncryptDataInputRequest, CryptoEncryptDataInputResponse>>,
    pub decrypt_data:
        OnceCell<RouterHandlers<CryptoDecryptDataInputRequest, CryptoDecryptDataInputResponse>>,

    pub acl: OnceCell<RouterHandlers<AclHandlerRequest, AclHandlerResponse>>,
    pub interest: OnceCell<RouterHandlers<InterestHandlerRequest, InterestHandlerResponse>>,
}

pub type RouterHandlersContainerRef = Arc<RouterHandlersContainer>;

impl RouterHandlersContainer {
    fn new(chain: RouterHandlerChain, storage: RouterHandlersStorage) -> Self {
        Self {
            chain,
            storage,

            put_object: OnceCell::new(),
            get_object: OnceCell::new(),
            post_object: OnceCell::new(),
            select_object: OnceCell::new(),
            delete_object: OnceCell::new(),

            get_data: OnceCell::new(),
            put_data: OnceCell::new(),
            delete_data: OnceCell::new(),

            sign_object: OnceCell::new(),
            verify_object: OnceCell::new(),
            encrypt_data: OnceCell::new(),
            decrypt_data: OnceCell::new(),

            acl: OnceCell::new(),
            interest: OnceCell::new(),
        }
    }

    pub fn put_object(
        &self,
    ) -> &RouterHandlers<NONPutObjectInputRequest, NONPutObjectInputResponse> {
        self.put_object.get_or_init(|| {
            RouterHandlers::<NONPutObjectInputRequest, NONPutObjectInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_put_object(
        &self,
    ) -> Option<&RouterHandlers<NONPutObjectInputRequest, NONPutObjectInputResponse>> {
        self.put_object.get()
    }

    pub fn get_object(
        &self,
    ) -> &RouterHandlers<NONGetObjectInputRequest, NONGetObjectInputResponse> {
        self.get_object.get_or_init(|| {
            RouterHandlers::<NONGetObjectInputRequest, NONGetObjectInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_get_object(
        &self,
    ) -> Option<&RouterHandlers<NONGetObjectInputRequest, NONGetObjectInputResponse>> {
        self.get_object.get()
    }

    pub fn post_object(
        &self,
    ) -> &RouterHandlers<NONPostObjectInputRequest, NONPostObjectInputResponse> {
        self.post_object.get_or_init(|| {
            RouterHandlers::<NONPostObjectInputRequest, NONPostObjectInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_post_object(
        &self,
    ) -> Option<&RouterHandlers<NONPostObjectInputRequest, NONPostObjectInputResponse>> {
        self.post_object.get()
    }

    pub fn select_object(
        &self,
    ) -> &RouterHandlers<NONSelectObjectInputRequest, NONSelectObjectInputResponse> {
        self.select_object.get_or_init(|| {
            RouterHandlers::<NONSelectObjectInputRequest, NONSelectObjectInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_select_object(
        &self,
    ) -> Option<&RouterHandlers<NONSelectObjectInputRequest, NONSelectObjectInputResponse>> {
        self.select_object.get()
    }

    pub fn delete_object(
        &self,
    ) -> &RouterHandlers<NONDeleteObjectInputRequest, NONDeleteObjectInputResponse> {
        self.delete_object.get_or_init(|| {
            RouterHandlers::<NONDeleteObjectInputRequest, NONDeleteObjectInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_delete_object(
        &self,
    ) -> Option<&RouterHandlers<NONDeleteObjectInputRequest, NONDeleteObjectInputResponse>> {
        self.delete_object.get()
    }

    pub fn put_data(&self) -> &RouterHandlers<NDNPutDataInputRequest, NDNPutDataInputResponse> {
        self.put_data.get_or_init(|| {
            RouterHandlers::<NDNPutDataInputRequest, NDNPutDataInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_put_data(
        &self,
    ) -> Option<&RouterHandlers<NDNPutDataInputRequest, NDNPutDataInputResponse>> {
        self.put_data.get()
    }

    pub fn get_data(&self) -> &RouterHandlers<NDNGetDataInputRequest, NDNGetDataInputResponse> {
        self.get_data.get_or_init(|| {
            RouterHandlers::<NDNGetDataInputRequest, NDNGetDataInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_get_data(
        &self,
    ) -> Option<&RouterHandlers<NDNGetDataInputRequest, NDNGetDataInputResponse>> {
        self.get_data.get()
    }

    pub fn delete_data(
        &self,
    ) -> &RouterHandlers<NDNDeleteDataInputRequest, NDNDeleteDataInputResponse> {
        self.delete_data.get_or_init(|| {
            RouterHandlers::<NDNDeleteDataInputRequest, NDNDeleteDataInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_delete_data(
        &self,
    ) -> Option<&RouterHandlers<NDNDeleteDataInputRequest, NDNDeleteDataInputResponse>> {
        self.delete_data.get()
    }

    // sign
    pub fn sign_object(
        &self,
    ) -> &RouterHandlers<CryptoSignObjectInputRequest, CryptoSignObjectInputResponse> {
        self.sign_object.get_or_init(|| {
            RouterHandlers::<CryptoSignObjectInputRequest, CryptoSignObjectInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_sign_object(
        &self,
    ) -> Option<&RouterHandlers<CryptoSignObjectInputRequest, CryptoSignObjectInputResponse>> {
        self.sign_object.get()
    }

    // verify
    pub fn verify_object(
        &self,
    ) -> &RouterHandlers<CryptoVerifyObjectInputRequest, CryptoVerifyObjectInputResponse> {
        self.verify_object.get_or_init(|| {
            RouterHandlers::<CryptoVerifyObjectInputRequest, CryptoVerifyObjectInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_verify_object(
        &self,
    ) -> Option<&RouterHandlers<CryptoVerifyObjectInputRequest, CryptoVerifyObjectInputResponse>>
    {
        self.verify_object.get()
    }

    // encrypt
    pub fn encrypt_data(
        &self,
    ) -> &RouterHandlers<CryptoEncryptDataInputRequest, CryptoEncryptDataInputResponse> {
        self.encrypt_data.get_or_init(|| {
            RouterHandlers::<CryptoEncryptDataInputRequest, CryptoEncryptDataInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_encrypt_data(
        &self,
    ) -> Option<&RouterHandlers<CryptoEncryptDataInputRequest, CryptoEncryptDataInputResponse>>
    {
        self.encrypt_data.get()
    }

    // decrypt
    pub fn decrypt_data(
        &self,
    ) -> &RouterHandlers<CryptoDecryptDataInputRequest, CryptoDecryptDataInputResponse> {
        self.decrypt_data.get_or_init(|| {
            RouterHandlers::<CryptoDecryptDataInputRequest, CryptoDecryptDataInputResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_decrypt_data(
        &self,
    ) -> Option<&RouterHandlers<CryptoDecryptDataInputRequest, CryptoDecryptDataInputResponse>>
    {
        self.decrypt_data.get()
    }

    // acl
    pub fn acl(&self) -> &RouterHandlers<AclHandlerRequest, AclHandlerResponse> {
        self.acl.get_or_init(|| {
            RouterHandlers::<AclHandlerRequest, AclHandlerResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_acl(&self) -> Option<&RouterHandlers<AclHandlerRequest, AclHandlerResponse>> {
        self.acl.get()
    }

    // interest
    pub fn interest(&self) -> &RouterHandlers<InterestHandlerRequest, InterestHandlerResponse> {
        self.interest.get_or_init(|| {
            RouterHandlers::<InterestHandlerRequest, InterestHandlerResponse>::new(
                self.chain.clone(),
                self.storage.clone(),
            )
        })
    }
    pub fn try_interest(
        &self,
    ) -> Option<&RouterHandlers<InterestHandlerRequest, InterestHandlerResponse>> {
        self.interest.get()
    }

    pub(crate) fn clear_dec_handlers(&self, dec_id: &Option<ObjectId>) -> bool {
        let mut changed = false;
        if let Some(container) = self.get_object.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }
        if let Some(container) = self.put_object.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }
        if let Some(container) = self.post_object.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }
        if let Some(container) = self.select_object.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }
        if let Some(container) = self.delete_object.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }

        if let Some(container) = self.get_data.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }
        if let Some(container) = self.put_data.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }
        if let Some(container) = self.delete_data.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }

        if let Some(container) = self.sign_object.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }
        if let Some(container) = self.verify_object.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }
        if let Some(container) = self.encrypt_data.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }
        if let Some(container) = self.decrypt_data.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }

        if let Some(container) = self.acl.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }

        if let Some(container) = self.interest.get() {
            changed |= container.clear_dec_handlers(dec_id)
        }

        changed
    }

    pub(crate) fn dump_data(&self) -> RouterHandlerContainerSavedData {
        let mut result = RouterHandlerContainerSavedData::new();
        if let Some(container) = self.get_object.get() {
            result.get_object = container.dump_data();
        }
        if let Some(container) = self.put_object.get() {
            result.put_object = container.dump_data();
        }
        if let Some(container) = self.post_object.get() {
            result.post_object = container.dump_data();
        }
        if let Some(container) = self.select_object.get() {
            result.select_object = container.dump_data();
        }
        if let Some(container) = self.delete_object.get() {
            result.delete_object = container.dump_data();
        }

        if let Some(container) = self.get_data.get() {
            result.get_data = container.dump_data();
        }
        if let Some(container) = self.put_data.get() {
            result.put_data = container.dump_data();
        }
        if let Some(container) = self.delete_data.get() {
            result.delete_data = container.dump_data();
        }

        if let Some(container) = self.sign_object.get() {
            result.sign_object = container.dump_data();
        }
        if let Some(container) = self.verify_object.get() {
            result.verify_object = container.dump_data();
        }
        if let Some(container) = self.encrypt_data.get() {
            result.sign_object = container.dump_data();
        }
        if let Some(container) = self.decrypt_data.get() {
            result.verify_object = container.dump_data();
        }

        if let Some(container) = self.acl.get() {
            result.acl = container.dump_data();
        }

        if let Some(container) = self.interest.get() {
            result.interest = container.dump_data();
        }

        result
    }

    pub(crate) fn load_data(&self, data: RouterHandlerContainerSavedData) {
        if let Some(list) = data.get_object {
            self.get_object().load_data(list);
        }
        if let Some(list) = data.put_object {
            self.put_object().load_data(list);
        }
        if let Some(list) = data.post_object {
            self.post_object().load_data(list);
        }
        if let Some(list) = data.select_object {
            self.select_object().load_data(list);
        }
        if let Some(list) = data.delete_object {
            self.delete_object().load_data(list);
        }

        if let Some(list) = data.get_data {
            self.get_data().load_data(list);
        }
        if let Some(list) = data.put_data {
            self.put_data().load_data(list);
        }
        if let Some(list) = data.delete_data {
            self.delete_data().load_data(list);
        }

        if let Some(list) = data.sign_object {
            self.sign_object().load_data(list);
        }
        if let Some(list) = data.verify_object {
            self.verify_object().load_data(list);
        }
        if let Some(list) = data.encrypt_data {
            self.encrypt_data().load_data(list);
        }
        if let Some(list) = data.decrypt_data {
            self.decrypt_data().load_data(list);
        }

        if let Some(list) = data.acl {
            self.acl().load_data(list);
        }

        if let Some(list) = data.interest {
            self.interest().load_data(list);
        }
    }
}

#[derive(Clone)]
pub struct RouterHandlersManager {
    storage: RouterHandlersStorage,
    acl_manager: AclManagerRef,

    pre_noc: Arc<RouterHandlersContainer>,
    post_noc: Arc<RouterHandlersContainer>,

    pre_router: Arc<RouterHandlersContainer>,
    post_router: Arc<RouterHandlersContainer>,

    pre_forward: Arc<RouterHandlersContainer>,
    post_forward: Arc<RouterHandlersContainer>,

    pre_crypto: Arc<RouterHandlersContainer>,
    post_crypto: Arc<RouterHandlersContainer>,

    handler: Arc<RouterHandlersContainer>,

    acl: Arc<RouterHandlersContainer>,

    ndn: Arc<RouterHandlersContainer>,
}

impl RouterHandlersManager {
    pub fn new(config_isolate: Option<String>, acl_manager: AclManagerRef) -> Self {
        let storage = RouterHandlersStorage::new(config_isolate);
        let ret = Self {
            storage: storage.clone(),
            acl_manager,

            pre_noc: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::PreNOC,
                storage.clone(),
            )),
            post_noc: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::PostNOC,
                storage.clone(),
            )),

            pre_router: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::PreRouter,
                storage.clone(),
            )),
            post_router: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::PostRouter,
                storage.clone(),
            )),

            pre_forward: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::PreForward,
                storage.clone(),
            )),
            post_forward: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::PostForward,
                storage.clone(),
            )),

            pre_crypto: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::PreCrypto,
                storage.clone(),
            )),
            post_crypto: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::PostCrypto,
                storage.clone(),
            )),

            handler: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::Handler,
                storage.clone(),
            )),

            acl: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::Acl,
                storage.clone(),
            )),

            ndn: Arc::new(RouterHandlersContainer::new(
                RouterHandlerChain::NDN,
                storage.clone(),
            )),
        };

        storage.bind(ret.clone());

        ret
    }

    pub fn acl_manager(&self) -> &AclManagerRef {
        &self.acl_manager
    }

    pub fn clone_processor(&self, source: RequestSourceInfo) -> RouterHandlerManagerProcessorRef {
        let sm = SharedRouterHandlersManager::new(self.clone(), source);

        Arc::new(Box::new(sm))
    }

    pub async fn load(&self) -> BuckyResult<()> {
        self.storage.load().await
    }

    pub fn handlers(&self, chain: &RouterHandlerChain) -> &Arc<RouterHandlersContainer> {
        match *chain {
            RouterHandlerChain::PreNOC => &self.pre_noc,
            RouterHandlerChain::PostNOC => &self.post_noc,

            RouterHandlerChain::PreRouter => &self.pre_router,
            RouterHandlerChain::PostRouter => &self.post_router,

            RouterHandlerChain::PreForward => &self.pre_forward,
            RouterHandlerChain::PostForward => &self.post_forward,

            RouterHandlerChain::PreCrypto => &self.pre_crypto,
            RouterHandlerChain::PostCrypto => &self.post_crypto,

            RouterHandlerChain::Handler => &self.handler,

            RouterHandlerChain::Acl => &self.acl,

            RouterHandlerChain::NDN => &self.ndn,
        }
    }

    pub(crate) fn clear_dec_handlers(&self, dec_id: &Option<ObjectId>) -> bool {
        let mut changed = self.pre_noc.clear_dec_handlers(dec_id);
        changed |= self.post_noc.clear_dec_handlers(dec_id);

        changed |= self.pre_router.clear_dec_handlers(dec_id);
        changed |= self.post_router.clear_dec_handlers(dec_id);

        changed |= self.pre_forward.clear_dec_handlers(dec_id);
        changed |= self.post_forward.clear_dec_handlers(dec_id);

        changed |= self.pre_crypto.clear_dec_handlers(dec_id);
        changed |= self.post_crypto.clear_dec_handlers(dec_id);

        changed |= self.handler.clear_dec_handlers(dec_id);

        changed |= self.acl.clear_dec_handlers(dec_id);

        changed |= self.ndn.clear_dec_handlers(dec_id);

        if changed {
            self.storage.async_save();
        }

        changed
    }

    pub(crate) fn dump_data(&self) -> RouterHandlersSavedData {
        let mut list = RouterHandlersSavedData::new();
        let data = self.pre_noc.dump_data();
        if !data.is_empty() {
            list.pre_noc = Some(data);
        }
        let data = self.post_noc.dump_data();
        if !data.is_empty() {
            list.post_noc = Some(data);
        }

        let data = self.pre_router.dump_data();
        if !data.is_empty() {
            list.pre_router = Some(data);
        }
        let data = self.post_router.dump_data();
        if !data.is_empty() {
            list.post_router = Some(data);
        }

        let data = self.pre_forward.dump_data();
        if !data.is_empty() {
            list.pre_forward = Some(data);
        }
        let data = self.post_forward.dump_data();
        if !data.is_empty() {
            list.post_forward = Some(data);
        }

        let data = self.pre_crypto.dump_data();
        if !data.is_empty() {
            list.pre_crypto = Some(data);
        }
        let data = self.post_crypto.dump_data();
        if !data.is_empty() {
            list.post_crypto = Some(data);
        }

        let data = self.acl.dump_data();
        if !data.is_empty() {
            list.acl = Some(data);
        }

        let data = self.ndn.dump_data();
        if !data.is_empty() {
            list.ndn = Some(data);
        }

        list
    }

    pub(crate) fn load_data(&self, list: RouterHandlersSavedData) {
        if let Some(data) = list.pre_noc {
            self.pre_noc.load_data(data);
        }
        if let Some(data) = list.post_noc {
            self.post_noc.load_data(data);
        }

        if let Some(data) = list.pre_router {
            self.pre_router.load_data(data);
        }
        if let Some(data) = list.post_router {
            self.post_router.load_data(data);
        }

        if let Some(data) = list.pre_forward {
            self.pre_forward.load_data(data);
        }
        if let Some(data) = list.post_forward {
            self.post_forward.load_data(data);
        }

        if let Some(data) = list.pre_crypto {
            self.pre_crypto.load_data(data);
        }
        if let Some(data) = list.post_crypto {
            self.post_crypto.load_data(data);
        }

        if let Some(data) = list.acl {
            self.acl.load_data(data);
        }

        if let Some(data) = list.ndn {
            self.ndn.load_data(data);
        }
    }

    pub async fn check_access(
        &self,
        source: &RequestSourceInfo,
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: &str,
        req_path: &Option<String>,
        filter: &Option<String>,
    ) -> BuckyResult<()> {
        let req_path = if chain == RouterHandlerChain::Handler {
            // Handler must specified valid req_path
            if req_path.is_none() {
                let msg = format!(
                    "{} {} handler's req_path should specify! id={}",
                    chain, category, id,
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }

            use std::str::FromStr;
            RequestGlobalStatePath::from_str(&req_path.as_ref().unwrap())?
        } else {
            if req_path.is_none() && filter.is_none() {
                let msg = format!(
                    "{} {} handler's req_path or filter should specify at least one! id={}",
                    chain, category, id,
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }

            let path = format!("{}/{}/{}/", CYFS_HANDLER_VIRTUAL_PATH, chain, category);
            RequestGlobalStatePath::new_system_dec(Some(path))
        };

        if source.is_current_zone() {
            if source.check_target_dec_permission(&req_path.dec_id) {
                return Ok(());
            }
        }

        self.acl_manager
            .global_state_meta()
            .check_access(source, &req_path, RequestOpType::Write)
            .await
    }
}

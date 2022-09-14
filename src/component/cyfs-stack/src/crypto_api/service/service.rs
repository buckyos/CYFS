use super::super::local::*;
use super::super::router::CryptoRouter;
use crate::acl::AclManagerRef;
use crate::crypto::*;
use crate::forward::ForwardProcessorManager;
use crate::meta::ObjectFailHandler;
use crate::router_handler::RouterHandlersManager;
use crate::zone::ZoneManagerRef;

pub struct CryptoService {
    crypto: ObjectCrypto,
    router: CryptoInputProcessorRef,
}

impl CryptoService {
    pub(crate) fn new(
        crypto: ObjectCrypto,
        acl: AclManagerRef,
        zone_manager: ZoneManagerRef,
        forward: ForwardProcessorManager,
        fail_handler: ObjectFailHandler,
        router_handlers: RouterHandlersManager,
    ) -> Self {
        let router = CryptoRouter::new_acl(
            acl,
            crypto.clone(),
            zone_manager,
            forward,
            fail_handler,
            router_handlers,
        );

        Self { crypto, router }
    }

    // 直接获取本地service，不带handler和acl
    pub(crate) fn local_service(&self) -> &ObjectCrypto {
        &self.crypto
    }

    pub fn clone_processor(&self) -> CryptoInputProcessorRef {
        self.router.clone()
    }
}

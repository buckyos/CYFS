use cyfs_lib::*;


pub(crate) struct CyfsStackProcessors {
    pub non_service: NONOutputProcessorRef,
    pub ndn_service: NDNOutputProcessorRef,
    pub crypto_service: CryptoOutputProcessorRef,
    pub util_service: UtilOutputProcessorRef,
    pub trans_service: TransOutputProcessorRef,

    pub router_handlers: RouterHandlerManagerProcessorRef,
    pub router_events: RouterEventManagerProcessorRef,

    pub root_state: GlobalStateOutputProcessorRef,
    pub root_state_access: GlobalStateAccessOutputProcessorRef,

    pub local_cache: GlobalStateOutputProcessorRef,
    pub local_cache_access: GlobalStateAccessOutputProcessorRef,

    pub root_state_meta: GlobalStateMetaOutputProcessorRef,
    pub local_cache_meta: GlobalStateMetaOutputProcessorRef,
}


impl UniCyfsStack for CyfsStackProcessors {
    fn non_service(&self) -> &NONOutputProcessorRef {
        &self.non_service
    }

    fn ndn_service(&self) -> &NDNOutputProcessorRef {
        &self.ndn_service
    }

    fn crypto_service(&self) -> &CryptoOutputProcessorRef {
        &self.crypto_service
    }

    fn util_service(&self) -> &UtilOutputProcessorRef {
        &self.util_service
    }

    fn trans_service(&self) -> &TransOutputProcessorRef {
        &self.trans_service
    }

    fn router_handlers(&self) -> &RouterHandlerManagerProcessorRef {
        &self.router_handlers
    }

    fn router_events(&self) -> &RouterEventManagerProcessorRef {
        &self.router_events
    }

    fn root_state(&self) -> &GlobalStateOutputProcessorRef {
        &self.root_state
    }

    fn root_state_access(&self) -> &GlobalStateAccessOutputProcessorRef {
        &self.root_state_access
    }

    fn local_cache(&self) -> &GlobalStateOutputProcessorRef {
        &self.local_cache
    }

    fn local_cache_access(&self) -> &GlobalStateAccessOutputProcessorRef {
        &self.local_cache_access
    }

    fn root_state_meta(&self) -> &GlobalStateMetaOutputProcessorRef {
        &self.root_state_meta
    }

    fn local_cache_meta(&self) -> &GlobalStateMetaOutputProcessorRef {
        &self.local_cache_meta
    }
}

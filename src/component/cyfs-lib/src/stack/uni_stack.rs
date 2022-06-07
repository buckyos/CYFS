use crate::*;

use std::sync::Arc;

pub trait UniCyfsStack: Send + Sync {
    fn non_service(&self) -> &NONOutputProcessorRef;
    fn ndn_service(&self) -> &NDNOutputProcessorRef;
    fn crypto_service(&self) -> &CryptoOutputProcessorRef;
    fn util_service(&self) -> &UtilOutputProcessorRef;
    fn trans_service(&self) -> &TransOutputProcessorRef;

    fn router_handlers(&self) -> &RouterHandlerManagerProcessorRef;
    fn router_events(&self) -> &RouterEventManagerProcessorRef;

    fn root_state(&self) -> &GlobalStateOutputProcessorRef;
    fn root_state_access(&self) -> &GlobalStateAccessOutputProcessorRef;

    fn local_cache(&self) -> &GlobalStateOutputProcessorRef;
    fn local_cache_access(&self) -> &GlobalStateAccessOutputProcessorRef;
}

pub type UniObjectStackRef = Arc<dyn UniCyfsStack>;

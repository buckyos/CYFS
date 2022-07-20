use cyfs_base::*;
use cyfs_bdt::{
    Stack, 
    NdnEventHandler, 
    DefaultNdnEventHandler, 
    ndn::channel::{Channel, UploadSession, protocol::{Interest, PieceData}}
};
use cyfs_util::acl::*;
use cyfs_lib::*;
use crate::{
    acl::*, 
    router_handler::RouterHandlersManager, 
    non_api::NONHandlerCaller
};
use super::{
    acl::BdtNdnDataAclProcessor
};


pub struct BdtNdnEventHandler {
    acl: BdtNdnDataAclProcessor, 
    handlers: RouterHandlersManager, 
    default: DefaultNdnEventHandler
}

impl BdtNdnEventHandler {
    pub fn new(acl: AclManagerRef, handlers: RouterHandlersManager) -> Self {
        Self { 
            acl: BdtNdnDataAclProcessor::new(acl, handlers.clone()), 
            handlers, 
            default: DefaultNdnEventHandler::new()
        }
    }
}

#[async_trait::async_trait]
impl NdnEventHandler for BdtNdnEventHandler {
    fn on_unknown_piece_data(
        &self, 
        stack: &Stack, 
        piece: &PieceData, 
        from: &Channel
    ) -> BuckyResult<()> {
        self.default.on_unknown_piece_data(stack, piece, from)
    }

    async fn on_newly_interest(
        &self, 
        stack: &Stack, 
        interest: &Interest, 
        from: &Channel
    ) -> BuckyResult<()> {

        let next_step = if let Some(handler) = self.handlers.handlers(&RouterHandlerChain::Interest) .try_interest() {
            if !handler.is_empty() {
                let mut param = RouterHandlerInterestRequest {
                    request: InterestHandlerRequest {
                        interest: interest.clone(), 
                        from_channel: from.remote().clone()
                    }, 
                    response: Some(Ok(InterestHandlerResponse::Upload))
                };
                // FIXME: how to emit handler
                // let mut handler = NONHandlerCaller::new(handler.emitter());
                // if let Some(resp) = handler.call("sign_object", &mut param).await? {
                //     resp?
                // } else {
                //     param.response.unwrap()
                // }
                InterestHandlerResponse::Upload
            } else {
                InterestHandlerResponse::Upload
            }
        } else {
            InterestHandlerResponse::Upload
        };

        match next_step {
            InterestHandlerResponse::Upload => {
                match self.acl.get_data(
                    BdtGetDataInputRequest {
                        object_id: interest.chunk.object_id(), 
                        source: from.remote().clone(), 
                        referer: interest.referer.clone() 
                    }).await {
                    Ok(_) => {
                        let _ = self.default.on_newly_interest(stack, interest, from).await?;
                    }, 
                    Err(err) => {
                        let session = UploadSession::canceled(interest.chunk.clone(), 
                                                        interest.session_id.clone(), 
                                                        interest.prefer_type.clone(), 
                                                        from.clone(), 
                                                        err.code());
                        let _ = from.upload(session.clone());
                        let _ = session.on_interest(interest)?;
                    }
                }
            }, 
            InterestHandlerResponse::Resp(resp_interest) => {
                from.resp_interest(resp_interest);
            }, 
            InterestHandlerResponse::Handled => {

            }
        }

       Ok(())
    }
}


use cyfs_base::*;
use cyfs_bdt::{
    Stack, 
    NdnEventHandler, 
    DefaultNdnEventHandler, 
    ndn::channel::{Channel, UploadSession, protocol::{Interest, RespInterest, PieceData}}
};
use cyfs_util::acl::*;
use cyfs_lib::*;
use crate::{
    acl::*, 
    router_handler::{RouterHandlers, RouterHandlersManager}, 
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

    async fn call_default_with_acl(&self, stack: &Stack, interest: &Interest, from: &Channel) -> BuckyResult<()> {
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
        Ok(())
    }
    

    async fn call_interest_handler(
        handler: &RouterHandlers<InterestHandlerRequest, InterestHandlerResponse>, 
        interest: &Interest, 
        from: &Channel) -> BuckyResult<(InterestHandlerRequest, InterestHandlerResponse)> {
        let referer = if let Some(referer) = interest.referer.as_ref() {
            Some(BdtDataRefererInfo::decode_string(referer.as_str())?)
        } else {
            None
        };
        
        let mut param = RouterHandlerInterestRequest {
            request: InterestHandlerRequest {
                session_id: interest.session_id.clone(), 
                chunk: interest.chunk.clone(),
                prefer_type: interest.prefer_type.clone(), 
                from: interest.from.clone(),
                referer, 
                from_channel: from.remote().clone()
            }, 
            response: None
        };

        let mut caller = NONHandlerCaller::new(handler.emitter());
        match caller.call("interest_handler", &mut param).await {
            Ok(resp) => {
                if let Some(resp) = resp {
                    resp.map(|resp| (param.request, resp))
                } else {
                    //RouterHandlerAction::Default
                    Ok((param.request, InterestHandlerResponse::Default))
                }
            }, 
            Err(err) => {
                match err.code() {
                    BuckyErrorCode::Reject => {
                        //RouterHandlerAction::Reject
                        Ok((param.request, InterestHandlerResponse::Resp(RespInterestFields {
                            err: BuckyErrorCode::Reject, 
                            redirect: None, 
                            redirect_referer_target: None
                        })))
                    }, 
                    BuckyErrorCode::Ignored => {
                        //RouterHandlerAction::Drop
                        Ok((param.request, InterestHandlerResponse::Handled))
                    }, 
                    _ => Err(err)
                }
            } 
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
       
        let handler = self.handlers.handlers(&RouterHandlerChain::NDN).try_interest();
        if handler.is_none() || handler.as_ref().unwrap().is_empty() {
             // no handler register
            self.call_default_with_acl(stack, interest, from).await
        } else {
            let (req, resp) = Self::call_interest_handler(handler.unwrap(), interest, from).await?;
            match resp {
                InterestHandlerResponse::Default => {
                    self.call_default_with_acl(stack, interest, from).await
                }, 
                InterestHandlerResponse::Upload => {
                    self.default.on_newly_interest(stack, interest, from).await
                },  
                InterestHandlerResponse::Transmit(to) => {
                    let mut interest = interest.clone();
                    if interest.from.is_none() {
                        interest.from = Some(from.remote().clone());
                    }
                    let trans_channel = stack.ndn().channel_manager().create_channel(&to);
                    trans_channel.interest(interest);
                    Ok(())      
                }, 
                InterestHandlerResponse::Resp(resp_fields) => {
                    let mut referer = req.referer.unwrap();
                    if resp_fields.redirect_referer_target.is_some() {
                        referer.target = resp_fields.redirect_referer_target;
                    }
                   
                    from.resp_interest(RespInterest {
                        session_id: interest.session_id.clone(), 
                        chunk: interest.chunk.clone(),  
                        err: resp_fields.err,
                        redirect: resp_fields.redirect, 
                        redirect_referer: Some(referer.encode_string())
                    });
                    Ok(())      
                }, 
                InterestHandlerResponse::Handled => {
                    Ok(())             
                }
            }     
        }
    }
}


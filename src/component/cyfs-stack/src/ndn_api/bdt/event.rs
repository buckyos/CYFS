use cyfs_base::*;
use cyfs_bdt::{
    Stack, 
    NdnEventHandler, 
    DefaultNdnEventHandler, 
    ndn::channel::{Channel, DownloadSession, protocol::v0::*}, 
    download
};
use cyfs_util::acl::*;
use cyfs_lib::*;
use crate::{
    acl::*, 
    zone::*, 
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
    pub fn new(zone_manager: ZoneManagerRef, acl: AclManagerRef, handlers: RouterHandlersManager) -> Self {
        Self { 
            acl: BdtNdnDataAclProcessor::new(zone_manager, acl, handlers.clone()), 
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
                from.resp_interest(RespInterest {
                    session_id: interest.session_id.clone(), 
                    chunk: interest.chunk.clone(),  
                    err: err.code(),
                    redirect: None,
                    redirect_referer: None,
                    to: None,
                });
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
    ) -> BuckyResult<DownloadSession> {
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
                InterestHandlerResponse::Upload(groups) => {
                    let _ = download::start_upload_task(stack, interest, from, groups).await?;
                    Ok(())
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
                        redirect_referer: Some(referer.encode_string()),
                        to: None
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


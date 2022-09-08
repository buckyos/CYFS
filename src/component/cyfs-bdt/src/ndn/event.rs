use cyfs_base::*;
use crate::{
    stack::{Stack}, 
};
use super::{
    scheduler::*, 
    channel::{
        protocol::v0::*, 
        Channel, 
        UploadSession, DownloadSession
    }, 
};


#[async_trait::async_trait]
pub trait NdnEventHandler: Send + Sync {
    async fn on_newly_interest(
        &self, 
        stack: &Stack, 
        interest: &Interest, 
        from: &Channel
    ) -> BuckyResult<()>;

    fn on_unknown_piece_data(
        &self, 
        stack: &Stack, 
        piece: &PieceData, 
        from: &Channel
    ) -> BuckyResult<DownloadSession>;
}


// 需要通知到stack层次的内部事件在这里统一实现；这里的代码属于策略，异变或者可以通过配置扩展
pub struct DefaultNdnEventHandler {
   
}

impl DefaultNdnEventHandler {
    pub fn new() -> Self {
        Self {
           
        }
    }
}

#[async_trait::async_trait]
impl NdnEventHandler for DefaultNdnEventHandler {
    fn on_unknown_piece_data(
        &self, 
        _stack: &Stack, 
        _piece: &PieceData, 
        _from: &Channel
    ) -> BuckyResult<DownloadSession> {
        Err(BuckyError::new(BuckyErrorCode::Interrupted, "no session downloading"))
    }

    // 处理全新的interest请求;已经正在上传的interest请求不会传递到这里;
    async fn on_newly_interest(
        &self, 
        stack: &Stack, 
        interest: &Interest, 
        from: &Channel
    ) -> BuckyResult<()> {

        let requestor = {
            if let Some(requestor) = &interest.from {
                if let Some(requestor) = stack.ndn().channel_manager().channel_of(&requestor) {
                    requestor
                } else {
                    let resp_interest = 
                        RespInterest {
                            session_id: interest.session_id.clone(),
                            chunk: interest.chunk.clone(),
                            err: BuckyErrorCode::NotConnected,
                            redirect: Some(stack.local_device_id().clone()),
                            redirect_referer: interest.referer.clone(),
                            to: Some(requestor.clone()),
                        };

                    from.resp_interest(resp_interest);
                    return Ok(());
                }
            } else {
                from.clone()
            }
        };

        //TODO: 这里的逻辑可能是：根据当前 root uploader的resource情况，
        //  如果还有空间或者透支不大，可以新建上传；
        //  否则拒绝或者给出其他源，回复RespInterest

        let session = {
            match stack.ndn().chunk_manager().start_upload(
                interest.session_id.clone(), 
                interest.chunk.clone(), 
                interest.prefer_type.clone(), 
                requestor.clone(), 
                stack.ndn().root_task().upload().resource().clone()).await {
                Ok(session) => {
                    // do nothing
                    Ok(session)
                }, 
                Err(err) => {
                    match err.code() {
                        BuckyErrorCode::AlreadyExists => {
                            //do nothing
                            Err(err)
                        },
                        _ => Ok(UploadSession::canceled(
                            interest.chunk.clone(), 
                            interest.session_id.clone(), 
                            interest.prefer_type.clone(), 
                            from.clone(), 
                            err.code()))
                    }
                }
            }?
        };

        // 加入到channel的 upload sessions中
        let _ = requestor.upload(session.clone());
        session.on_interest(interest)

    }
}

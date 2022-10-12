use cyfs_base::*;
use crate::{
    stack::{Stack}, 
};
use super::{
    upload::*, 
    channel::{
        protocol::v0::*, 
        Channel, 
        UploadSession, 
        DownloadSession
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

    pub async fn start_upload_task(
        stack: &Stack, 
        interest: &Interest, 
        to: &Channel, 
        group: &UploadGroup, 
        path: Option<String>, 
    ) -> BuckyResult<UploadSession> {
        let cache = stack.ndn().chunk_manager().create_cache(&interest.chunk);
        let desc = interest.prefer_type.fill_values(&interest.chunk);
        let encoder = cache.create_encoder(&desc);
        let session = UploadSession::new(
            interest.chunk.clone(), 
            interest.session_id.clone(), 
            desc.clone(), 
            encoder, 
            to.clone());
        let _ = group.add(path, session.clone_as_task());
        // 加入到channel的 upload sessions中
        let _ = to.upload(session.clone());
        let _ = session.on_interest(interest)?;
        Ok(session)
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

        let _ = Self::start_upload_task(
            stack, 
            interest, 
            &requestor, 
            stack.ndn().root_task().upload(), 
            None).await?;

        Ok(())
    }
}

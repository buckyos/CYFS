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
        // 加入到channel的 upload sessions中
        let session = to.upload(
            interest.chunk.clone(), 
        interest.session_id.clone(), 
        desc.clone(), 
            encoder)?;
        let _ = group.add_task(path, session.clone_as_task());
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
        
        let _ = Self::start_upload_task(
            stack, 
            interest, 
            &from, 
            stack.ndn().root_task().upload(), 
            None
        ).await?;

        Ok(())
    }
}

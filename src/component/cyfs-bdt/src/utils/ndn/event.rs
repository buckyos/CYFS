use cyfs_base::*;
use crate::{
    stack::{Stack}, 
    ndn::{*, channel::{*, protocol::v0::*}, chunk::{ChunkStreamCache, RawCache}}
};


pub async fn start_upload_task(
    stack: &Stack, 
    interest: &Interest, 
    to: &Channel, 
    owners: Vec<String>, 
) -> BuckyResult<Box<dyn UploadTask>> {
    let desc = interest.prefer_type.fill_values(&interest.chunk);
    let cache = stack.ndn().chunk_manager().create_cache(&interest.chunk);
    let encoder = cache.create_encoder(&desc);
   
    
    let session = to.upload(
        interest.chunk.clone(), 
        interest.session_id.clone(), 
        desc.clone(), 
        encoder)?;
    
    let _ = stack.ndn().root_task().upload().create_sub_task(owners, &session)?;
  
    Ok(session.clone_as_task())
}

pub async fn start_upload_task_from_cache<T: RawCache + 'static>(
    stack: &Stack, 
    interest: &Interest, 
    to: &Channel, 
    owners: Vec<String>, 
    cache: T
) -> BuckyResult<Box<dyn UploadTask>> {
    let desc = interest.prefer_type.fill_values(&interest.chunk);

    let stream_cache = ChunkStreamCache::new(&interest.chunk);
    stream_cache.load(true, Box::new(cache))?;
    let encoder = stream_cache.create_encoder(&desc);

     
    let session = to.upload(
        interest.chunk.clone(), 
        interest.session_id.clone(), 
        desc.clone(), 
        encoder)?;
    
    let _ = stack.ndn().root_task().upload().create_sub_task(owners, &session)?;
  
    Ok(session.clone_as_task())
}


// 需要通知到stack层次的内部事件在这里统一实现；这里的代码属于策略，异变或者可以通过配置扩展
#[derive(Clone)]
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
        
        let _ = start_upload_task(
            stack, 
            interest, 
            &from, 
            vec![],
        ).await?;

        Ok(())
    }
}

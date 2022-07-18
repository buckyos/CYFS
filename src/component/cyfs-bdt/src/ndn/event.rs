use cyfs_base::*;
use crate::{
    stack::{Stack}, 
};
use super::{
    scheduler::*, 
    channel::{
        protocol::*, 
        Channel, 
        UploadSession
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
    ) -> BuckyResult<()>;
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
    ) -> BuckyResult<()> {
        //FIXME: 也有可能向上传递新建task
        Err(BuckyError::new(BuckyErrorCode::Interrupted, "no session downloading"))
    }

    // 处理全新的interest请求;已经正在上传的interest请求不会传递到这里;
    async fn on_newly_interest(
        &self, 
        stack: &Stack, 
        interest: &Interest, 
        from: &Channel
    ) -> BuckyResult<()> {

        // let session = match self.acl.get_data(
        //     BdtGetDataInputRequest {
        //         object_id: interest.chunk.object_id(), 
        //         source: from.remote().clone(), 
        //         referer: interest.referer.clone() 
        //     }).await {
        //     Ok(_) => {
                
        //     }, 
        //     Err(err) => {
        //         Ok(UploadSession::canceled(interest.chunk.clone(), 
        //                                           interest.session_id.clone(), 
        //                                           interest.prefer_type.clone(), 
        //                                           from.clone(), 
        //                                           err.code()))
        //     }
        // }?;


        //TODO: 这里的逻辑可能是：根据当前 root uploader的resource情况，
        //  如果还有空间或者透支不大，可以新建上传；
        //  否则拒绝或者给出其他源，回复RespInterest
        let session = match stack.ndn().chunk_manager().start_upload(
            interest.session_id.clone(), 
            interest.chunk.clone(), 
            interest.prefer_type.clone(), 
            from.clone(), 
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
        }?;
        // 加入到channel的 upload sessions中
        let _ = from.upload(session.clone());
        session.on_interest(interest)

        // let stack = self.stack();
        // match r {
        //     BdtEventResult::UploadProcess => {
                
        //     },
        //     BdtEventResult::RedirectProcess(target_id, referer) => {
        //         return Ok(UploadSession::redirect(interest.chunk.clone(), 
        //                                           interest.session_id.clone(),
        //                                           interest.prefer_type.clone(),
        //                                           from.clone(),
        //                                           target_id.clone(),
        //                                           referer));
        //     },
        //     BdtEventResult::WaitRedirectProcess(target_id) => {
        //         let config = Arc::new(ChunkDownloadConfig::force_stream(target_id));
        //         let chunk = interest.chunk.clone();
        //         let stack = stack.clone();
        //         // download process will be initialized from the source node, and channel will wait
        //         async_std::task::spawn( async move {
        //             let config = config.clone();
        //             let chunk = chunk.clone();
        //             let ndc = stack.ndn().chunk_manager().ndc();
        //             let chunk_task = ChunkTask::new(stack.to_weak(),
        //                                             chunk,
        //                                             config,
        //                                             vec![Box::new(MemChunkStore::new(ndc))],
        //                                             stack.ndn().root_task().download().resource().clone(),
		// 				    None);
        //             chunk_task.start();

        //             loop {
        //                 match chunk_task.schedule_state() {
        //                     TaskState::Pending | TaskState::Running(_) => {
        //                         let _ = async_std::future::timeout(std::time::Duration::from_secs(1), async_std::future::pending::<()>()).await;
        //                     },
        //                     _ => { break; }
        //                 }
        //             }
        //         });
        //         return Ok(UploadSession::wait_redirect(interest.chunk.clone(),
        //                                                 interest.session_id.clone(),
        //                                                 interest.prefer_type.clone(),
        //                                                 from.clone()));

        //     }
    }
}


use std::{sync::Arc, convert::TryFrom};

use cyfs_base::*;
use cyfs_util::acl::*;
use crate::{
    stack::{WeakStack, Stack}, MemChunkStore
};
use super::{
    ChunkTask,
    scheduler::*, 
    chunk::ChunkDownloadConfig,
    channel::{
        protocol::v0::*, 
        Channel, 
        UploadSession
    }, event_ext::EventExtHandler, 
};


struct DefaultAcl {

}

#[async_trait::async_trait]
impl BdtDataAclProcessor for DefaultAcl {
    async fn get_data(&self, _req: BdtGetDataInputRequest) -> BuckyResult<()> {
        Ok(())
    }   

    async fn put_data(&self, _req: BdtPutDataInputRequest) -> BuckyResult<()> {
        Ok(())
    }

    async fn delete_data(&self, _req: BdtDeleteDataInputRequest) -> BuckyResult<()>{
        Ok(())
    }
}

// 需要通知到stack层次的内部事件在这里统一实现；这里的代码属于策略，异变或者可以通过配置扩展
pub struct EventHandler {
    stack: WeakStack, 
    acl: Box<dyn BdtDataAclProcessor>,
    event_ext: EventExtHandler,
}

impl EventHandler {
    pub fn new(stack: WeakStack, 
               acl: Option<Box<dyn BdtDataAclProcessor>>, 
               event_mgr: Option<Box<dyn BdtEventHandleProcessor>>) -> Self {
        Self {
            stack: stack.clone(), 
            acl: acl.unwrap_or(Box::new(DefaultAcl {})),
            event_ext: EventExtHandler::new(stack.clone(), event_mgr),
        }
    }
    // 处理全新的interest请求;已经正在上传的interest请求不会传递到这里;
    pub async fn on_newly_interest(&self, interest: &Interest, from: &Channel) -> BuckyResult<UploadSession> {
        //TODO: 这里的逻辑可能是：根据当前 root uploader的resource情况，
        //  如果还有空间或者透支不大，可以新建上传；
        //  否则拒绝或者给出其他源，回复RespInterest
        match self.acl.get_data(
            BdtGetDataInputRequest {
                object_id: interest.chunk.object_id(), 
                source: from.remote().clone(), 
                referer: interest.referer.clone() 
            }).await {
            Ok(_) => {}, 
            Err(err) => {
                return Ok(UploadSession::canceled(interest.chunk.clone(), 
                                                  interest.session_id.clone(), 
                                                  interest.prefer_type.clone(), 
                                                  from.clone(), 
                                                  err.code()));
            }
        }

        let r = {
            match self.event_ext.on_newly_interest(interest, from).await {
                Ok(r) => r,
                Err(err) => {
                    return Ok(UploadSession::canceled(interest.chunk.clone(), 
                                                    interest.session_id.clone(), 
                                                    interest.prefer_type.clone(), 
                                                    from.clone(), 
                                                    err.code()));
                }
            }
        };

        let stack = self.stack();
        match r {
            BdtEventResult::UploadProcess => {
                match stack.ndn().chunk_manager().start_upload(
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
                }
            },
            BdtEventResult::RedirectProcess(target_id, referer) => {
                return Ok(UploadSession::redirect(interest.chunk.clone(), 
                                                  interest.session_id.clone(),
                                                  interest.prefer_type.clone(),
                                                  from.clone(),
                                                  target_id.clone(),
                                                  referer));
            },
            BdtEventResult::WaitRedirectProcess(target_id) => {
                let config = Arc::new(ChunkDownloadConfig::force_stream(target_id));
                let chunk = interest.chunk.clone();
                let stack = stack.clone();
                // download process will be initialized from the source node, and channel will wait
                async_std::task::spawn( async move {
                    let config = config.clone();
                    let chunk = chunk.clone();
                    let ndc = stack.ndn().chunk_manager().ndc();
                    let chunk_task = ChunkTask::new(stack.to_weak(),
                                                    chunk,
                                                    config,
                                                    vec![Box::new(MemChunkStore::new(ndc))],
                                                    stack.ndn().root_task().download().resource().clone(),
						    None);
                    chunk_task.start();

                    loop {
                        match chunk_task.schedule_state() {
                            TaskState::Pending | TaskState::Running(_) => {
                                let _ = async_std::future::timeout(std::time::Duration::from_secs(1), async_std::future::pending::<()>()).await;
                            },
                            _ => { break; }
                        }
                    }
                });
                return Ok(UploadSession::wait_redirect(interest.chunk.clone(),
                                                        interest.session_id.clone(),
                                                        interest.prefer_type.clone(),
                                                        from.clone()));

            }
        }
    }

    pub fn on_unknown_piece_data(&self, _piece: &PieceData, _from: &Channel) -> BuckyResult<UploadSession> {
        //FIXME: 也有可能向上传递新建task
        Err(BuckyError::new(BuckyErrorCode::Interrupted, "no session downloading"))
    }

    fn stack(&self) -> Stack {
        Stack::from(&self.stack)
    }
}

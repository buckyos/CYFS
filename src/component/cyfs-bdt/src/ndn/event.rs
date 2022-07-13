
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
    }, 
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

struct DefaultRefer;
#[async_trait::async_trait]
impl BdtRefererProcessor for DefaultRefer {
    async fn parse_referer_link(&self, _referer: &str) -> BuckyResult<(ObjectId /* target-id */, String /* inner */)> {
        Ok((ObjectId::default(), Default::default()))
    }

    async fn build_referer_link(&self, _target_id: &ObjectId, _inner: String) -> BuckyResult<String> {
        Ok(Default::default())
    }

}

// 需要通知到stack层次的内部事件在这里统一实现；这里的代码属于策略，异变或者可以通过配置扩展
pub struct EventHandler {
    stack: WeakStack, 
    acl: Box<dyn BdtDataAclProcessor>,
    referer: Box<dyn BdtRefererProcessor>,
}

impl EventHandler {
    pub fn new(stack: WeakStack, acl: Option<Box<dyn BdtDataAclProcessor>>, refer: Option<Box<dyn BdtRefererProcessor>>) -> Self {
        Self {
            stack, 
            acl: acl.unwrap_or(Box::new(DefaultAcl {})),
            referer: refer.unwrap_or(Box::new(DefaultRefer{})),
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

        let (target_id, chunk) = {
            if let Some(referer) = &interest.referer {
                match self.referer.parse_referer_link(referer.as_str()).await {
                    Ok((target_id, chunk)) => { (Some(DeviceId::try_from(&target_id)?), Some(chunk)) }
                    Err(err) => {
                        return Ok(UploadSession::canceled(interest.chunk.clone(), 
                                                          interest.session_id.clone(), 
                                                          interest.prefer_type.clone(), 
                                                          from.clone(), 
                                                          err.code()));
                    }
                }
            } else { (None, None) }
        };

        let stack = self.stack();
        if stack.local()
                .connect_info()
                .dump_current_pn()
                .map_or(false, |id| {
                    *id == *stack.local_device_id()
                }) {
            // cache-node
            // start upload process
            match stack.ndn().chunk_manager().start_upload(interest.session_id.clone(), 
                                                           interest.chunk.clone(), 
                                                           interest.prefer_type.clone(), 
                                                           from.clone(), 
                                                           stack.ndn().root_task().upload().resource().clone()).await {
                Ok(session) => { return Ok(session); },
                Err(err) => {
                    match err.code() {
                        BuckyErrorCode::NotFound => {
                            let target_Id = target_id.unwrap().clone();
                            let config = Arc::new(ChunkDownloadConfig::force_stream(target_Id));
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
                        },
                        BuckyErrorCode::Pending => {
                            return Ok(UploadSession::wait_redirect(interest.chunk.clone(),
                                                                   interest.session_id.clone(),
                                                                   interest.prefer_type.clone(),
                                                                   from.clone()));
                        }
                        BuckyErrorCode::AlreadyExists => { return Err(err); },
                        _ => {
                            return Ok(UploadSession::canceled(interest.chunk.clone(), 
                                                              interest.session_id.clone(), 
                                                              interest.prefer_type.clone(), 
                                                              from.clone(), 
                                                              err.code()));        
                        }
                    }
                }
            }
        } else {
            // 源机
            if let Some(dump_pn) = stack.local().connect_info().dump_current_pn() {
                return Ok(UploadSession::redirect(interest.chunk.clone(), 
                                                  interest.session_id.clone(),
                                                  interest.prefer_type.clone(),
                                                  from.clone(),
                                                  dump_pn.clone(),
                                                  self.referer
                                                               .build_referer_link(stack.local_device_id().object_id(), interest.chunk.to_string()).await?));
            } else {
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

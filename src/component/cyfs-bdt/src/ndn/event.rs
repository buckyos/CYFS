use cyfs_base::*;
use cyfs_util::acl::*;
use crate::{
    stack::{WeakStack, Stack}
};
use super::{
    scheduler::*, 
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

// 需要通知到stack层次的内部事件在这里统一实现；这里的代码属于策略，异变或者可以通过配置扩展
pub struct EventHandler {
    stack: WeakStack, 
    acl: Box<dyn BdtDataAclProcessor>
}

impl EventHandler {
    pub fn new(stack: WeakStack, acl: Option<Box<dyn BdtDataAclProcessor>>) -> Self {
        Self {
            stack, 
            acl: acl.unwrap_or(Box::new(DefaultAcl {}))
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
            Ok(_) => {
                let stack = self.stack();
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
            Err(err) => {
                Ok(UploadSession::canceled(
                    interest.chunk.clone(), 
                    interest.session_id.clone(), 
                    interest.prefer_type.clone(), 
                    from.clone(), 
                    err.code()))
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

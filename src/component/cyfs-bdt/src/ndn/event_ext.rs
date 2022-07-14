
use ::cyfs_util::acl::*;
use cyfs_base::*;

use crate::{
    stack::{WeakStack, Stack}, MemChunkStore
};

use super::{
    // ChunkTask,
    scheduler::*, 
    // chunk::ChunkDownloadConfig,
    channel::{
        protocol::*, 
        Channel, 
        UploadSession
    }, 
};

struct BdtEventHandleProcessorDefault;

#[async_trait::async_trait]
impl BdtEventHandleProcessor for BdtEventHandleProcessorDefault {
    async fn get_handle(&self, _: &DeviceId, _: ObjectId) -> Option<Box<dyn BdtEventHandleTrait>> {
        None
    }
}

pub struct EventExtHandler {
    stack: WeakStack, 
    event_handle_mgr: Box<dyn BdtEventHandleProcessor>,
}

impl EventExtHandler {
    pub fn new(stack: WeakStack, event: Option<Box<dyn BdtEventHandleProcessor>>) -> Self {
        Self {
            stack, 
            event_handle_mgr: event.unwrap_or(Box::new(BdtEventHandleProcessorDefault))
        }
    }

    pub async fn on_newly_interest(&self, interest: &Interest, from: &Channel) -> BuckyResult<BdtEventResult> {
        let h = self.event_handle_mgr
                                                        .get_handle(from.remote(), interest.chunk.object_id()).await;

        match h {
            Some(h) => {
                h.on_newly_interest(BdtEventRequest{object_id: interest.chunk.object_id(),
                                                             from: from.remote().clone(),
                                                             referer: interest.referer.clone()}).await
            }
            None => Ok(BdtEventResult::UploadProcess)
        }
    }
}


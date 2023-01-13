use cyfs_base::{BuckyResult, ObjectId};
use cyfs_bdt::Stack;
use cyfs_core::GroupRPath;
use cyfs_lib::NONObjectInfo;

use crate::CHANNEL_CAPACITY;

pub struct ReplyWaiter<T> {
    rx: async_std::channel::Receiver<T>,
    seq: u64,
}

impl<T> ReplyWaiter<T> {
    pub fn wait(&self) -> async_std::channel::Recv<'_, T> {
        self.rx.recv()
    }
}

impl<T> Drop for ReplyWaiter<T> {
    fn drop(&mut self) {
        todo!("distach the tx")
    }
}

pub struct Listener {
    wait_seq: u64,
}

impl Listener {
    pub fn new(vport: u16, bdt_stack: Stack) {}

    pub fn listen(&self) {}

    pub async fn wait_proposal_result(
        &self,
        proposal_id: ObjectId,
    ) -> BuckyResult<ReplyWaiter<BuckyResult<Option<NONObjectInfo>>>> {
        let (tx_proposal_result, rx_proposal_result) =
            async_std::channel::bounded::<BuckyResult<Option<NONObjectInfo>>>(CHANNEL_CAPACITY);
        unimplemented!()
    }

    pub async fn wait_query_state(
        &self,
        sub_path: String,
        rpath: GroupRPath,
    ) -> BuckyResult<ReplyWaiter<BuckyResult<ObjectId>>> {
        unimplemented!()
    }
}

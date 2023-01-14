use cyfs_base::{BuckyResult, ObjectId};
use cyfs_bdt::Stack;
use cyfs_core::GroupRPath;
use cyfs_lib::NONObjectInfo;

use crate::{dec_state::CallReplyWaiter, CHANNEL_CAPACITY};

pub struct Listener {
    wait_seq: u64,
}

impl Listener {
    pub fn new(vport: u16, bdt_stack: Stack) {}

    pub fn listen(&self) {}

    pub async fn wait_query_state(
        &self,
        sub_path: String,
        rpath: GroupRPath,
    ) -> BuckyResult<CallReplyWaiter<BuckyResult<ObjectId>>> {
        unimplemented!()
    }
}

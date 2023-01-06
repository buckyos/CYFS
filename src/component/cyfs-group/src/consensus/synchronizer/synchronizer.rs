use cyfs_base::{BuckyResult, ObjectId};
use cyfs_core::GroupConsensusBlock;

pub struct Synchronizer {}

impl Synchronizer {
    pub fn spawn(
        network_sender: crate::network::Sender,
        rpath: GroupRPath,
        rx_message: Receiver<(HotstuffMessage, ObjectId)>,
    ) -> Self {
        Self {}
    }

    pub fn sync_with_height(
        &self,
        min_height: u64,
        max_height: u64,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        unimplemented!()
    }

    pub fn sync_with_round(
        &self,
        min_round: u64,
        max_round: u64,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        unimplemented!()
    }

    pub fn push_outorder_block(
        &self,
        block: &GroupConsensusBlock,
        min_round: u64,
        remote: ObjectId,
    ) -> BuckyResult<()> {
        unimplemented!()
    }

    pub fn pop_link_from(&self, block: &GroupConsensusBlock) -> BuckyResult<()> {
        unimplemented!()
    }
}


struct SynchronizerRunner {

}

impl SynchronizerRunner {
    async fn run(&mut self) {
        
    }
}
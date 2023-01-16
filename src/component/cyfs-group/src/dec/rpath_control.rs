use std::sync::Arc;

use async_std::sync::Mutex;
use cyfs_base::{BuckyResult, Group, NamedObject, ObjectDesc, ObjectId};
use cyfs_chunk_lib::ChunkMeta;
use cyfs_core::{GroupProposal, GroupRPath, GroupRPathStatus};
use cyfs_lib::NONObjectInfo;

struct RPathControlRaw {
    network: crate::network::Sender,
    
}

impl RPathControlRaw {
    pub fn rpath(&self) -> &GroupRPath {
        unimplemented!()
    }

    async fn push_proposal(&self, proposal: GroupProposal) -> BuckyResult<()> {
        unimplemented!()
    }

    pub fn select_branch(&self, block_id: ObjectId, source: ObjectId) -> BuckyResult<()> {
        unimplemented!()
    }

}

pub struct RPathControl {
    raw: Arc<RPathControlRaw>,
}

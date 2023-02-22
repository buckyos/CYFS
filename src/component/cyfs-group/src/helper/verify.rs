use cyfs_base::{BuckyResult, Group, NamedObject, ObjectDesc};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockDesc, GroupConsensusBlockObject, HotstuffBlockQC,
};

use crate::GroupRPathStatus;

pub async fn verify_block(
    block_desc: &GroupConsensusBlockDesc,
    qc: &HotstuffBlockQC,
    group: &Group,
) -> BuckyResult<bool> {
    let block_id = block_desc.object_id();
    if qc.round != block_desc.content().round() || qc.block_id != block_id {
        log::error!(
            "the qc-block({}) should be next block({})",
            qc.round,
            block_id
        );
        return Ok(false);
    }

    unimplemented!()
}

pub async fn verify_rpath_value(
    value: &GroupRPathStatus,
    sub_path: &str,
    block_desc: &GroupConsensusBlockDesc,
    qc: &HotstuffBlockQC,
    group: &Group,
) -> BuckyResult<bool> {
    unimplemented!()
}

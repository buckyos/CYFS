use cyfs_base::{BuckyResult, Group, NamedObject, ObjectDesc};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockObject, GroupRPathStatus, HotstuffBlockQC,
};

pub async fn verify_block(
    block: &GroupConsensusBlock,
    qc: &HotstuffBlockQC,
    group: &Group,
) -> BuckyResult<bool> {
    let block_id = block.named_object().desc().object_id();
    if qc.round != block.round() || qc.block_id != block_id {
        log::error!(
            "the qc-block({}) should be next block({})",
            qc.round,
            block_id
        );
        return Ok(false);
    }

    if !block.check() {
        return Ok(false);
    }

    unimplemented!()
}

pub async fn verify_rpath_value(
    value: &GroupRPathStatus,
    sub_path: &str,
    block: &GroupConsensusBlock,
    qc: &HotstuffBlockQC,
    group: &Group,
) -> BuckyResult<bool> {
    unimplemented!()
}

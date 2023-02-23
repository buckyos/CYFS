use cyfs_base::{BuckyResult, Group, NamedObject, ObjectDesc};
use cyfs_core::{
    GroupConsensusBlock, GroupConsensusBlockDesc, GroupConsensusBlockObject, HotstuffBlockQC,
};

use crate::GroupRPathStatus;

pub async fn verify_rpath_value(
    value: &GroupRPathStatus,
    sub_path: &str,
    block_desc: &GroupConsensusBlockDesc,
    qc: &HotstuffBlockQC,
    group: &Group,
) -> BuckyResult<bool> {
    unimplemented!()
}

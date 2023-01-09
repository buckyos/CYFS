use cyfs_base::{BuckyResult, GroupId};
use cyfs_core::DecAppId;

use crate::GroupDecControl;

pub struct GroupManager {}

impl GroupManager {
    pub fn get_group_dec_control(
        &self,
        group_id: GroupId,
        dec_id: DecAppId,
        r_path: &str,
    ) -> BuckyResult<GroupDecControl> {
        unimplemented!()
    }
}

// pub struct GroupMemberState {
//     None,
//     Joined,
//     Crave(CraveObject),
//     Invite(InviteObject),
//     Reject(RejectCraveObject | RejectInviteObject),
//     Removed(RemoveMemberObject)
// }

// pub struct GroupControl {
//     pub fn member_state(member_id: ObjectId) -> BuckyResult<GroupMemberState> {

//     }

//     pub fn invite(target: ObjectId) -> BuckyResult<GroupMemberState> {

//     }

//     pub fn crave() -> BuckyResult<GroupMemberState> {

//     }

//     pub fn remove_member(target: ObjectId) -> BuckyResult<GroupMemberState> {

//     }

//     pub fn add_invite_handle(() -> GroupMemberState) {

//     }

//     pub fn add_crave_handle(() -> GroupMemberState) {

//     }
// }

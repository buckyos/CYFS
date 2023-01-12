use cyfs_base::ObjectId;
use cyfs_bdt::Stack;
use cyfs_core::GroupRPath;

use crate::HotstuffMessage;

#[derive(Clone)]
pub struct Sender {}

impl Sender {
    pub fn new(vport: u16, bdt_stack: Stack) {}

    pub(crate) async fn post_message(
        &self,
        msg: HotstuffMessage,
        rpath: GroupRPath,
        to: &ObjectId,
    ) {
    }

    pub(crate) async fn broadcast(&self, msg: HotstuffMessage, rpath: GroupRPath, to: &[ObjectId]) {
    }
}

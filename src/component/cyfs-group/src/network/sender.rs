use cyfs_base::ObjectId;
use cyfs_bdt::Stack;
use cyfs_core::GroupRPath;

use crate::HotstuffMessage;

pub(crate) struct Sender {}

impl Sender {
    pub fn new(vport: u16, bdt_stack: Stack) {}

    pub async fn post_package(&self, msg: HotstuffMessage, rpath: GroupRPath, to: &ObjectId) {}

    pub async fn broadcast(&self, msg: HotstuffMessage, rpath: GroupRPath, to: &[ObjectId]) {}
}

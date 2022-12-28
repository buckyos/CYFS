use cyfs_base::{BuckyResult, ObjectId};
use cyfs_lib::NONObjectInfo;

pub struct NonDriver {}

impl NonDriver {
    pub fn get_object(
        &self,
        object_id: &ObjectId,
        from: Option<&ObjectId>,
    ) -> BuckyResult<NONObjectInfo> {
        unimplemented!()
    }

    pub fn post_object(&self, obj: NONObjectInfo, to: &ObjectId) -> BuckyResult<()> {
        unimplemented!()
    }

    pub fn broadcast(&self, obj: NONObjectInfo, to: &[ObjectId]) -> BuckyResult<()> {
        unimplemented!()
    }
}

use cyfs_base::*;
use crate::{
    types::*
};
use super::super::{
    types::*
};

#[derive(Clone, Copy)]
pub enum UploadTaskPriority {
    Backgroud, 
    Normal, 
    Realtime(u32/*min speed*/),
}

impl Default for UploadTaskPriority {
    fn default() -> Self {
        Self::Normal
    }
}



#[async_trait::async_trait]
pub trait UploadTask: NdnTask {
    fn clone_as_upload_task(&self) -> Box<dyn UploadTask>;

    fn add_task(&self, _path: Option<String>, _sub: Box<dyn UploadTask>) -> BuckyResult<()> {
        Err(BuckyError::new(BuckyErrorCode::NotSupport, "no implement"))
    }
    fn sub_task(&self, _path: &str) -> Option<Box<dyn UploadTask>> {
        None
    }

    fn calc_speed(&self, when: Timestamp) -> u32;
}
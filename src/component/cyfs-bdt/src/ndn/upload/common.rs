use cyfs_base::*;
use crate::{
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


// 对scheduler的接口
#[derive(Debug)]
pub enum UploadTaskState {
    Uploading(u32/*速度*/),
    Paused,
    Error(BuckyError/*被cancel的原因*/), 
    Finished
}

#[derive(Clone, Debug)]
pub enum UploadTaskControlState {
    Normal, 
    Paused, 
    Canceled, 
}


#[async_trait::async_trait]
pub trait UploadTask: Send + Sync {
    fn clone_as_task(&self) -> Box<dyn UploadTask>;
    fn state(&self) -> UploadTaskState;
    async fn wait_finish(&self) -> UploadTaskState;
    fn control_state(&self) -> UploadTaskControlState;

    fn resume(&self) -> BuckyResult<UploadTaskControlState> {
        Ok(UploadTaskControlState::Normal)
    }
    fn cancel(&self) -> BuckyResult<UploadTaskControlState> {
        Ok(UploadTaskControlState::Normal)
    }
    fn pause(&self) -> BuckyResult<UploadTaskControlState> {
        Ok(UploadTaskControlState::Normal)
    }
    
    fn close(&self) -> BuckyResult<()> {
        Ok(())
    }

    fn add_task(&self, _path: Option<String>, _sub: Box<dyn UploadTask>) -> BuckyResult<()> {
        Err(BuckyError::new(BuckyErrorCode::NotSupport, "no implement"))
    }
    fn sub_task(&self, _path: &str) -> Option<Box<dyn UploadTask>> {
        None
    }

    fn calc_speed(&self, when: Timestamp) -> u32;
    fn cur_speed(&self) -> u32;
    fn history_speed(&self) -> u32;
}
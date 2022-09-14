use async_std::{
    sync::Arc, 
};
use crate::Timestamp;
use super::{
    channel::*, 
    download::*,
    upload::*
};

struct RootTaskImpl {
    max_download_speed: u32, 
    download: DownloadGroup, 
    upload: UploadGroup
}

#[derive(Clone)]
pub struct RootTask(Arc<RootTaskImpl>);

impl RootTask {
    pub fn new(max_download_speed: u32, history_speed: HistorySpeedConfig) -> Self {
        Self(Arc::new(RootTaskImpl {
            max_download_speed, 
            download: DownloadGroup::new(history_speed.clone(), None), 
            upload: UploadGroup::new(history_speed, None)
        }))
    }

    pub fn upload(&self) -> &UploadGroup {
        &self.0.upload
    }

    pub fn download(&self) -> &DownloadGroup {
        &self.0.download
    }

    pub fn on_schedule(&self, now: Timestamp) {
        self.download().calc_speed(now);
        self.download().on_drain(self.0.max_download_speed);
        self.upload().calc_speed(now);
    }
}



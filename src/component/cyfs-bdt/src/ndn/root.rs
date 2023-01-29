use async_std::{
    sync::Arc, 
};
use cyfs_base::*;
use crate::{
    types::*
};
use super::{
    types::*, 
    download::*,
    upload::*
};

struct RootTaskImpl {
    max_download_speed: u32, 
    download: DownloadRoot, 
    upload: UploadRoot
}

pub struct DownloadRoot {
    id_gen: IncreaseIdGenerator, 
    sub: DownloadGroup
}

impl DownloadRoot {
    fn next_index(&self) -> String {
        self.id_gen.generate().to_string()
    }

    pub fn makesure_path(
        &self, 
        path: String
    ) -> BuckyResult<(Box<dyn DownloadTask>, String, Option<String>)> {
        if path.len() == 0 {
            return Ok((self.sub.clone_as_download_task(), "/".to_owned(), None));
        } 

        let mut parts: Vec<&str> = path.split("/").collect();
        if parts.len() == 0 {
            return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "invalid group path"))
        } 
        
        let last_part = if parts[parts.len() - 1].len() == 0 {
            None 
        } else {
            Some(parts[parts.len() - 1].to_owned())
        };

        parts.remove(parts.len() - 1);

        let parent_path = (parts.join("/") + "/").to_owned();
        let mut parent = self.sub.clone_as_download_task();
        for part in parts {
            if let Some(sub) = parent.sub_task(part) {
                parent = sub;
            } else {
                let sub = DownloadGroup::new(self.sub.history_config().clone());
                parent.add_task(Some(part.to_owned()), sub.clone_as_download_task())?;
                parent = sub.clone_as_download_task();
            }
        }
        Ok((parent, parent_path, last_part))
    }


    pub fn add_task(&self, path: String, task: &dyn DownloadTask) -> BuckyResult<String> {
        let (parent, parent_path, rel_path) = self.makesure_path(path)?;
        let rel_path = rel_path.unwrap_or(self.next_index());
        let _ = parent.add_task(Some(rel_path.clone()), task.clone_as_download_task())?;
        let abs_path = [parent_path, rel_path].join("");
        task.on_post_add_to_root(abs_path.clone());
        Ok(abs_path)
    }

    pub fn sub_task(&self, path: &str) -> Option<Box<dyn DownloadTask>> {
        let abs_path = if path.starts_with("/") {
            &path[1..]
        } else {
            path
        };
        self.sub.sub_task(abs_path)
    }
}


pub struct UploadRoot {
    id_gen: IncreaseIdGenerator, 
    sub: UploadGroup
}

impl UploadRoot {
    fn next_index(&self) -> String {
        self.id_gen.generate().to_string()
    }

    pub fn makesure_path(
        &self, 
        path: String
    ) -> BuckyResult<(Box<dyn UploadTask>, String, Option<String>)> {
        if path.len() == 0 {
            return Ok((self.sub.clone_as_upload_task(), "/".to_owned(), None));
        } 

        let mut parts: Vec<&str> = path.split("/").collect();
        if parts.len() == 0 {
            return Err(BuckyError::new(BuckyErrorCode::InvalidInput, "invalid group path"))
        } 
        
        let last_part = if parts[parts.len() - 1].len() == 0 {
            None 
        } else {
            Some(parts[parts.len() - 1].to_owned())
        };

        parts.remove(parts.len() - 1);

        let parent_path = (parts.join("/") + "/").to_owned();
        let mut parent = self.sub.clone_as_upload_task();
        for part in parts {
            if let Some(sub) = parent.sub_task(part) {
                parent = sub;
            } else {
                let sub = UploadGroup::new(self.sub.history_config().clone());
                parent.add_task(Some(part.to_owned()), sub.clone_as_upload_task())?;
                parent = sub.clone_as_upload_task();
            }
        }
        Ok((parent, parent_path, last_part))
    }


    pub fn add_task(&self, pathes: Vec<String>, task: &dyn UploadTask) -> BuckyResult<Vec<String>> {
        let mut pathes = pathes;
        if pathes.len() == 0 {
            pathes.push("".to_owned())
        }

        let mut results = vec![];
        for path in pathes {
            if let Ok(abs_path) = self.makesure_path(path).and_then(|(parent, parent_path, rel_path)| {
                let rel_path = rel_path.unwrap_or(self.next_index());
                parent.add_task(Some(rel_path.clone()), task.clone_as_upload_task())
                    .map(|_| [parent_path, rel_path].join(""))
            }) {
                results.push(abs_path);
            }
        }
        
        if results.len() > 0 {
            Ok(results)
        } else {
            Err(BuckyError::new(BuckyErrorCode::Failed, ""))
        }
    }

    pub fn sub_task(&self, path: &str) -> Option<Box<dyn UploadTask>> {
        let abs_path = if path.starts_with("/") {
            &path[1..]
        } else {
            path
        };
        self.sub.sub_task(abs_path)
    }
}

#[derive(Clone)]
pub struct RootTask(Arc<RootTaskImpl>);

impl RootTask {
    pub fn new(max_download_speed: u32, history_speed: HistorySpeedConfig) -> Self {
        Self(Arc::new(RootTaskImpl {
            max_download_speed, 
            download: DownloadRoot {
                sub: DownloadGroup::new(history_speed.clone()), 
                id_gen: IncreaseIdGenerator::new()
            }, 
            upload: UploadRoot {
                sub: UploadGroup::new(history_speed.clone()), 
                id_gen: IncreaseIdGenerator::new()
            }
        }))
    }

    pub fn upload(&self) -> &UploadRoot {
        &self.0.upload
    }

    pub fn download(&self) -> &DownloadRoot {
        &self.0.download
    }

    pub fn on_schedule(&self, now: Timestamp) {
        self.download().sub.calc_speed(now);
        self.upload().sub.calc_speed(now);
    }
}



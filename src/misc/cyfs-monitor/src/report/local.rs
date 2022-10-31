use crate::def::*;
use cyfs_base::*;
use std::path::PathBuf;
use std::str::FromStr;
use async_std::fs::OpenOptions;
use async_std::prelude::*;

pub struct LocalStore {
    dir: PathBuf,
}

impl LocalStore {
    pub fn get_root() -> PathBuf {
        let root;
        #[cfg(target_os = "windows")]
        {
            root = "C:\\cyfs";
        }
        #[cfg(not(target_os = "windows"))]
        {
            root = "/cyfs";
        }

        PathBuf::from_str(root).unwrap()
    }
    pub fn new() -> Self {
        let root = Self::get_root();
        let dir = root.join("log").join("error");
        if !dir.is_dir() {
            std::fs::create_dir_all(&dir).unwrap();
        }
        
        Self {
            dir,
        }
    }

    pub async fn write(&self, info: &MonitorErrorInfo) -> BuckyResult<()> {
        let file = self.dir.join(&info.service);

        let mut f = OpenOptions::new().create(true).append(true).open(&file).await.map_err(|e| {
            let msg = format!("open local file error! file={}, {}", file.display(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let content = format!("dcfs monitor service error! \nservice:{}\ncode:{:?}\nmsg:{}", info.service, info.error.code(), info.error.msg());
        f.write(content.as_bytes()).await.map_err(|e| {
            let msg = format!("write to local file error! file={}, {}", file.display(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        f.flush().await.map_err(|e| {
            let msg = format!("flush local file error! file={}, {}", file.display(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl BugReporter for  LocalStore {
    async fn report_error(&self, info: &MonitorErrorInfo) -> BuckyResult<()> {
        self.write(info).await
    }
}
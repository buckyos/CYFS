use crate::archive::*;
use cyfs_base::*;
use super::restorer::*;

use std::path::{Path, PathBuf};
use async_std::io::WriteExt;

pub struct StackLocalObjectRestorer {
    cyfs_root: PathBuf,
}

impl StackLocalObjectRestorer {
    pub fn new(cyfs_root: PathBuf) -> Self {
        Self {
            cyfs_root,
        }
    }

    async fn restore_file(
        &self,
        inner_path: &Path,
        data: ObjectArchiveInnerFileData,
    ) -> BuckyResult<()> {
        if inner_path.is_absolute() {
            let msg = format!("invalid restore file's inner path! {}", inner_path.display());
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let full_path = self.cyfs_root.join(inner_path);
        if let Some(parent) = full_path.parent() {
            if !parent.is_dir() {
                async_std::fs::create_dir_all(&parent).await.map_err(|e| {
                    let msg = format!(
                        "create restore file's dir error: {}, err={}",
                        parent.display(),
                        e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;
            }
        }

        let mut opt = async_std::fs::OpenOptions::new();
        opt.write(true).create(true).truncate(true);

        let mut outfile = opt.open(&full_path).await.map_err(|e| {
            let msg = format!("create file error: {}, err={}", full_path.display(), e);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let s = data.into_stream();
        async_std::io::copy(s, outfile.clone()).await.map_err(|e| {
            let msg = format!(
                "write data to restore file error: {}, err={}",
                full_path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        outfile.flush().await.map_err(|e| {
            let msg = format!(
                "flush data to restore file error: {}, err={}",
                full_path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!("restore local file complete! file={}", full_path.display());
        Ok(())
    }

    pub async fn restore_object(&self) -> BuckyResult<()> {
        todo!();
    }
}


#[async_trait::async_trait]
impl ObjectRestorer for StackLocalObjectRestorer  {
    async fn restore_file(
        &self,
        inner_path: &Path,
        data: ObjectArchiveInnerFileData,
    ) -> BuckyResult<()> {
        Self::restore_file(&self, inner_path, data).await
    }
}
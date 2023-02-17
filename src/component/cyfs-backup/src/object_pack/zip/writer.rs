use super::super::pack::*;
use cyfs_base::*;

use async_std::io::Read as AsyncRead;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub struct ZipObjectPackWriter {
    path: PathBuf,
    writer: Option<zip::ZipWriter<File>>,
    options: zip::write::FileOptions,
    total_bytes_added: u64,
}

impl ZipObjectPackWriter {
    pub fn new(path: PathBuf) -> Self {
        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);

        Self {
            path,
            writer: None,
            options,
            total_bytes_added: 0,
        }
    }

    pub fn file_path(&self) -> &Path {
        &self.path
    }

    pub fn zip_inner_path(object_id: &ObjectId) -> String {
        let id36 = object_id.to_base36();
        let (name, dir) = id36.split_at(id36.len() - 3);
        let dir = match dir {
            "con" | "aux" | "nul" | "prn" => std::borrow::Cow::Owned(format!("{}_", dir)),
            _ => std::borrow::Cow::Borrowed(dir),
        };

        //let full_file_path = format!("{}/{}/{}", self.root_dir, dir, id36);
        let full_file_path = format!("{}/{}", dir, name);
        full_file_path
    }

    pub fn total_bytes_added(&self) -> u64 {
        self.total_bytes_added
    }

    pub fn open(&mut self) -> BuckyResult<()> {
        if self.path.is_file() {
            warn!(
                "zip file already exists! now been truncated and overwritten! file={}",
                self.path.display()
            );
        }

        let mut opt = std::fs::OpenOptions::new();
        if self.path.is_file() {
            opt.write(true).truncate(true);
        } else {
            opt.write(true).create_new(true);
        };

        let file = opt.open(&self.path).map_err(|e| {
            let msg = format!("open zip file failed! file={}, {}", self.path.display(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let writer = zip::ZipWriter::new(file);
        assert!(self.writer.is_none());
        self.writer = Some(writer);

        self.total_bytes_added = 0;

        Ok(())
    }

    pub fn add_data(&mut self, object_id: &ObjectId, data: &mut impl Read) -> BuckyResult<u64> {
        let writer = self.writer.as_mut().unwrap();

        let full_file_path = Self::zip_inner_path(object_id);

        writer
            .start_file(&full_file_path, self.options.clone())
            .map_err(|e| {
                let msg = format!(
                    "add data to zip failed! id={}, file={}, {}",
                    object_id, full_file_path, e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::Failed, msg)
            })?;

        let bytes = std::io::copy(data, writer).map_err(|e| {
            let msg = format!(
                "write file to zip failed! id={}, file={}, {}",
                object_id, full_file_path, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        self.total_bytes_added += bytes;

        Ok(bytes)
    }

    pub async fn flush(&mut self) -> BuckyResult<u64> {
        let writer = self.writer.as_mut().unwrap();

        writer.flush().map_err(|e| {
            let msg = format!("flush zip file failed! file={}, {}", self.path.display(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let meta = async_std::fs::metadata(&self.path).await.map_err(|e| {
            let msg = format!(
                "get metadata of zip file failed! file={}, {}",
                self.path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        Ok(meta.len())
    }

    pub fn finish(&mut self) -> BuckyResult<()> {
        let mut writer = self.writer.take().unwrap();

        writer.finish().map_err(|e| {
            let msg = format!(
                "finish zip file failed! file={}, {}",
                self.path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!("zip file finished! file={}", self.path.display());

        Ok(())
    }
}

#[async_trait::async_trait]
impl ObjectPackWriter for ZipObjectPackWriter {
    async fn open(&mut self) -> BuckyResult<()> {
        Self::open(&mut self)
    }
    
    fn total_bytes_added(&self) -> u64 {
        Self::total_bytes_added(&self)
    }

    fn file_path(&self) -> &Path {
        Self::file_path(&self)
    }

    async fn add_data(
        &mut self,
        object_id: &ObjectId,
        data: Box<dyn AsyncRead + Unpin + Send + 'static>,
    ) -> BuckyResult<u64> {
        let mut data = cyfs_util::async_read_to_sync(data);
        self.add_data(object_id, &mut data)
    }

    async fn flush(&mut self) -> BuckyResult<u64> {
        Self::flush(&mut self).await
    }

    async fn finish(&mut self) -> BuckyResult<()> {
        Self::finish(&mut self)
    }
}

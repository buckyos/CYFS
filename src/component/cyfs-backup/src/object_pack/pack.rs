use cyfs_base::*;

use async_std::io::Read as AsyncRead;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub type ObjectPackInnerFile = Box<dyn AsyncRead + Unpin + Sync + Send + 'static>;

#[async_trait::async_trait]
pub trait ObjectPackReader: Send {
    async fn open(&mut self) -> BuckyResult<()>;
    async fn close(&mut self) -> BuckyResult<()>;

    async fn get_data(&mut self, object_id: &ObjectId) -> BuckyResult<Option<ObjectPackInnerFile>>;

    async fn reset(&mut self);
    async fn next_data(&mut self) -> BuckyResult<Option<(ObjectId, ObjectPackInnerFile)>>;
}

#[async_trait::async_trait]
pub trait ObjectPackWriter: Send {
    fn total_bytes_added(&self) -> u64;

    fn file_path(&self) -> &Path;

    async fn open(&mut self) -> BuckyResult<()>;
    async fn add_data(
        &mut self,
        object_id: &ObjectId,
        data: Box<dyn AsyncRead + Unpin + Send + 'static>,
    ) -> BuckyResult<u64>;

    async fn flush(&mut self) -> BuckyResult<u64>;

    async fn finish(&mut self) -> BuckyResult<()>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectPackFileInfo {
    pub name: String,
    pub hash: HashValue,
    pub file_len: u64,
    pub data_len: u64,
}

#[derive(Debug, Clone, Copy)]
pub enum ObjectPackFormat {
    Zip,
}

pub struct ObjectPackFactory {}

impl ObjectPackFactory {
    pub fn create_reader(format: ObjectPackFormat, path: PathBuf) -> Box<dyn ObjectPackReader> {
        match format {
            ObjectPackFormat::Zip => {
                let ret = super::zip::ZipObjectPackReader::new(path);
                Box::new(ret)
            }
        }
    }

    pub fn create_writer(format: ObjectPackFormat, path: PathBuf) -> Box<dyn ObjectPackWriter> {
        match format {
            ObjectPackFormat::Zip => {
                let ret = super::zip::ZipObjectPackWriter::new(path);
                Box::new(ret)
            }
        }
    }

    pub fn create_zip_reader(path: PathBuf) -> Box<dyn ObjectPackReader> {
        Self::create_reader(ObjectPackFormat::Zip, path)
    }

    pub fn create_zip_writer(path: PathBuf) -> Box<dyn ObjectPackWriter> {
        Self::create_writer(ObjectPackFormat::Zip, path)
    }
}

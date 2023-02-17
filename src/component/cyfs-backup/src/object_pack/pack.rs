use cyfs_base::*;

use async_std::io::Read as AsyncRead;
use std::path::PathBuf;

pub type ObjectPackArchiveFile = Box<dyn AsyncRead + Unpin + Sync + Send + 'static>;

#[async_trait::async_trait]
pub trait ObjectPackReader: Send {
    async fn open(&mut self) -> BuckyResult<()>;
    async fn close(&mut self) -> BuckyResult<()>;

    async fn get_data(
        &mut self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectPackArchiveFile>>;

    async fn reset(&mut self);
    async fn next_data(&mut self) -> BuckyResult<Option<(ObjectId, ObjectPackArchiveFile)>>;
}

#[async_trait::async_trait]
pub trait ObjectPackWriter: Send {
    async fn total_bytes_added(&mut self) -> u64;

    async fn open(&mut self) -> BuckyResult<()>;
    async fn add_data(
        &mut self,
        object_id: &ObjectId,
        data: Box<dyn AsyncRead + Unpin + Send + 'static>,
    ) -> BuckyResult<u64>;

    async fn flush(&mut self) -> BuckyResult<u64>;

    async fn finish(&mut self) -> BuckyResult<()>;
}

pub enum ObjectPackFormat {
    Zip,
}

pub struct ObjectPackFactory {
    
}

impl ObjectPackFactory {
    pub fn create_reader(format: ObjectPackFormat, path: PathBuf) -> Box<dyn ObjectPackReader> {
        match format {
            ObjectPackFormat::Zip => {
                let ret=  super::zip::ZipObjectPackReader::new(path);
                Box::new(ret)
            }
        }
    }

    pub fn create_writer(format: ObjectPackFormat, path: PathBuf) -> Box<dyn ObjectPackWriter> {
        match format {
            ObjectPackFormat::Zip => {
                let ret=  super::zip::ZipObjectPackWriter::new(path);
                Box::new(ret)
            }
        }
    }
}
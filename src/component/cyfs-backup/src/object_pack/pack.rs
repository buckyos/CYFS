use cyfs_backup_lib::*;
use cyfs_base::*;

use async_std::io::Read as AsyncRead;
use async_std::io::ReadExt;
use std::path::{Path, PathBuf};

pub enum ObjectPackInnerFileData {
    Buffer(Vec<u8>),
    Stream(Box<dyn AsyncRead + Unpin + Sync + Send + 'static>),
}

impl ObjectPackInnerFileData {
    pub fn into_stream(self) -> Box<dyn AsyncRead + Unpin + Sync + Send + 'static> {
        match self {
            Self::Buffer(buf) => Box::new(async_std::io::Cursor::new(buf)),
            Self::Stream(stream) => stream,
        }
    }

    pub async fn into_buffer(self) -> BuckyResult<Vec<u8>> {
        match self {
            Self::Buffer(buf) => Ok(buf),
            Self::Stream(mut stream) => {
                let mut buf = vec![];
                stream.read_to_end(&mut buf).await.map_err(|e| {
                    let msg = format!("read stream to buffer failed! {}", e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;

                Ok(buf)
            }
        }
    }
}

pub struct ObjectPackInnerFile {
    pub data: ObjectPackInnerFileData,
    pub meta: Option<Vec<u8>>,
}

#[async_trait::async_trait]
pub trait ObjectPackReader: Send + Sync {
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
        data: Box<dyn AsyncRead + Unpin + Send + Sync + 'static>,
        meta: Option<Vec<u8>>,
    ) -> BuckyResult<BuckyResult<u64>>;

    async fn add_data_buf(
        &mut self,
        object_id: &ObjectId,
        data: &[u8],
        meta: Option<Vec<u8>>,
    ) -> BuckyResult<BuckyResult<u64>>;

    async fn flush(&mut self) -> BuckyResult<u64>;

    async fn finish(&mut self) -> BuckyResult<()>;
}

pub struct ObjectPackFactory {}

impl ObjectPackFactory {
    pub fn create_reader(
        format: ObjectPackFormat,
        path: PathBuf,
        crypto: Option<AesKey>,
    ) -> Box<dyn ObjectPackReader> {
        let reader = match format {
            ObjectPackFormat::Zip => {
                let ret = super::zip::ZipObjectPackReader::new(path);
                Box::new(ret)
            }
        };

        match crypto {
            Some(aes_key) => {
                let ret = super::aes::AesObjectPackReader::new(aes_key, reader);
                Box::new(ret)
            }
            None => reader,
        }
    }

    pub fn create_writer(
        format: ObjectPackFormat,
        path: PathBuf,
        crypto: Option<AesKey>,
    ) -> Box<dyn ObjectPackWriter> {
        let writer = match format {
            ObjectPackFormat::Zip => {
                let ret = super::zip::ZipObjectPackWriter::new(path);
                Box::new(ret)
            }
        };

        match crypto {
            Some(aes_key) => {
                let ret = super::aes::AesObjectPackWriter::new(aes_key, writer);
                Box::new(ret)
            }
            None => writer,
        }
    }

    pub fn create_zip_reader(path: PathBuf, crypto: Option<AesKey>) -> Box<dyn ObjectPackReader> {
        Self::create_reader(ObjectPackFormat::Zip, path, crypto)
    }

    pub fn create_zip_writer(path: PathBuf, crypto: Option<AesKey>) -> Box<dyn ObjectPackWriter> {
        Self::create_writer(ObjectPackFormat::Zip, path, crypto)
    }
}

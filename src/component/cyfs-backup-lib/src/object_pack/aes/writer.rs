use super::super::pack::*;
use cyfs_base::*;

use async_std::io::{Read as AsyncRead, ReadExt};
use std::path::Path;

pub struct AesObjectPackWriter {
    aes_key: AesKey,
    next: Box<dyn ObjectPackWriter>,

    cache_buf: Vec<u8>,
}

impl AesObjectPackWriter {
    pub fn new(aes_key: AesKey, next: Box<dyn ObjectPackWriter>) -> Self {
        Self {
            aes_key,
            next,
            cache_buf: Vec::with_capacity(1024 * 1024 * 4),
        }
    }

    async fn add_cache_data(
        &mut self,
        object_id: &ObjectId,
        meta: Option<Vec<u8>>,
    ) -> BuckyResult<BuckyResult<u64>> {
        let len = self.cache_buf.len();
        let pad_len = AesKey::padded_len(len);

        self.cache_buf.resize(pad_len, 0);
        let encrypt_len = self.aes_key.inplace_encrypt(&mut self.cache_buf, len)?;

        assert_eq!(encrypt_len, self.cache_buf.len());
        unsafe {
            self.cache_buf.set_len(encrypt_len);
        }

        // info!("encrypt data={:?}", self.cache_buf);
        // info!("encrypt data: {} -> {}", len, encrypt_len);

        let meta = match meta {
            Some(mut meta) => {
    
                let len = meta.len();
                let pad_len = AesKey::padded_len(len);
                meta.resize(pad_len, 0);

                let encrypt_len = self.aes_key.inplace_encrypt(&mut meta, len)?;
                unsafe {
                    meta.set_len(encrypt_len);
                }

                Some(meta)
            }
            None => None,
        };

        self.next
            .add_data_buf(object_id, &self.cache_buf, meta)
            .await
    }
}

#[async_trait::async_trait]
impl ObjectPackWriter for AesObjectPackWriter {
    async fn open(&mut self) -> BuckyResult<()> {
        self.next.open().await
    }

    fn total_bytes_added(&self) -> u64 {
        self.next.total_bytes_added()
    }

    fn file_path(&self) -> &Path {
        self.next.file_path()
    }

    async fn add_data(
        &mut self,
        object_id: &ObjectId,
        mut data: Box<dyn AsyncRead + Unpin + Send + Sync + 'static>,
        meta: Option<Vec<u8>>,
    ) -> BuckyResult<BuckyResult<u64>> {
        unsafe {
            self.cache_buf.set_len(0);
        }

        if let Err(e) = data.read_to_end(&mut self.cache_buf).await {
            return Ok(Err(e.into()));
        }

        self.add_cache_data(object_id, meta).await
    }

    async fn add_data_buf(
        &mut self,
        object_id: &ObjectId,
        data: &[u8],
        meta: Option<Vec<u8>>,
    ) -> BuckyResult<BuckyResult<u64>> {
        unsafe {
            self.cache_buf.set_len(0);
        }

        self.cache_buf.extend_from_slice(&data);

        self.add_cache_data(object_id, meta).await
    }

    async fn flush(&mut self) -> BuckyResult<u64> {
        self.next.flush().await
    }

    async fn finish(&mut self) -> BuckyResult<()> {
        self.next.finish().await
    }
}

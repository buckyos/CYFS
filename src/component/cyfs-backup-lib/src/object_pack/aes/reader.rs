use super::super::pack::*;
use cyfs_base::*;


pub struct AesObjectPackReader {
    aes_key: AesKey,
    next: Box<dyn ObjectPackReader>,
}

impl AesObjectPackReader {
    pub fn new(aes_key: AesKey, next: Box<dyn ObjectPackReader>) -> Self {
        Self {
            aes_key,
            next,
        }
    }

    async fn decrypt_inner_file(
        &self,
        info: ObjectPackInnerFile,
    ) -> BuckyResult<ObjectPackInnerFile> {
        let mut buf = info.data.into_buffer().await?;
        let len = buf.len();

        let decrypt_len = self.aes_key.inplace_decrypt(&mut buf, len)?;
        unsafe {
            buf.set_len(decrypt_len);
        }

        // info!("decrypt len={}, data={:?}", decrypt_len, buf);

        let meta = match info.meta {
            Some(mut meta) => {
                let len = meta.len();
                let decrypt_len = self.aes_key.inplace_decrypt(&mut meta, len)?;
                unsafe {
                    meta.set_len(decrypt_len);
                }

                Some(meta)
            }
            None => None,
        };

        let info = ObjectPackInnerFile {
            data: ObjectPackInnerFileData::Buffer(buf),
            meta,
        };

        Ok(info)
    }
}

#[async_trait::async_trait]
impl ObjectPackReader for AesObjectPackReader {
    async fn open(&mut self) -> BuckyResult<()> {
        self.next.open().await
    }

    async fn close(&mut self) -> BuckyResult<()> {
        self.next.close().await
    }

    async fn get_data(&mut self, object_id: &ObjectId) -> BuckyResult<Option<ObjectPackInnerFile>> {
        let ret = self.next.get_data(object_id).await?;
        let ret = match ret {
            Some(info) => Some(self.decrypt_inner_file(info).await?),
            None => None,
        };

        Ok(ret)
    }

    async fn reset(&mut self) {
        self.next.reset().await
    }
    async fn next_data(&mut self) -> BuckyResult<Option<(ObjectId, ObjectPackInnerFile)>> {
        let ret = self.next.next_data().await?;
        let ret = match ret {
            Some((object_id, info)) => {
                let info = self.decrypt_inner_file(info).await?;
                Some((object_id, info))
            }
            None => None,
        };

        Ok(ret)
    }
}

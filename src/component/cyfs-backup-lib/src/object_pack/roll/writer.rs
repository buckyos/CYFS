use super::super::pack::*;
use cyfs_base::*;

use async_std::io::Read as AsyncRead;
use std::path::PathBuf;

pub struct ObjectPackRollWriter {
    format: ObjectPackFormat,
    root: PathBuf,
    base_file_name: String,
    current_index: usize,
    current: Option<Box<dyn ObjectPackWriter>>,
    size_limit: u64,
    total_bytes_before_flush: u64,
    file_list: Vec<ObjectPackFileInfo>,
}

impl ObjectPackRollWriter {
    pub fn new(
        format: ObjectPackFormat,
        root: PathBuf,
        base_file_name: &str,
        size_limit: u64,
    ) -> Self {
        Self {
            format,
            root,
            base_file_name: base_file_name.to_owned(),
            current_index: 0,
            current: None,
            size_limit,
            total_bytes_before_flush: 0,
            file_list: vec![],
        }
    }

    pub fn into_file_list(self) -> Vec<ObjectPackFileInfo> {
        self.file_list
    }

    async fn open(&mut self) -> BuckyResult<()> {
        let file_name = format!("{}.{}.data", self.base_file_name, self.current_index);
        let file_path = self.root.join(&file_name);

        info!("new object pack file: {}", file_path.display());

        let mut writer = ObjectPackFactory::create_writer(self.format, file_path);
        writer.open().await?;

        self.current = Some(writer);
        Ok(())
    }

    pub async fn add_data(
        &mut self,
        object_id: &ObjectId,
        data: Box<dyn AsyncRead + Unpin + Send + Sync + 'static>,
        meta: Option<Vec<u8>>,
    ) -> BuckyResult<BuckyResult<u64>> {
        if self.current.is_none() {
            self.open().await?;
        }

        let writer = self.current.as_mut().unwrap();
        let ret = writer.add_data(object_id, data, meta).await?;
        drop(writer);

        if let Ok(bytes) = &ret {
            self.on_add_bytes(*bytes).await?;
        }

        Ok(ret)
    }

    pub async fn add_data_buf(
        &mut self,
        object_id: &ObjectId,
        data: &[u8],
        meta: Option<Vec<u8>>,
    ) -> BuckyResult<BuckyResult<u64>> {
        if self.current.is_none() {
            self.open().await?;
        }

        let writer = self.current.as_mut().unwrap();
        let ret = writer.add_data_buf(object_id, data, meta).await?;
        drop(writer);

        if let Ok(bytes) = &ret {
            self.on_add_bytes(*bytes).await?;
        }

        Ok(ret)
    }

    async fn on_add_bytes(&mut self, bytes: u64) -> BuckyResult<()> {
        self.total_bytes_before_flush += bytes;

        if self.total_bytes_before_flush > 1024 * 1024 {
            let writer = self.current.as_mut().unwrap();
            let file_size = writer.flush().await?;
            self.total_bytes_before_flush = 0;

            if file_size > self.size_limit {
                info!("object pack file extend limit size, now will step to next one! root={}, current_index={}", 
                    self.root.display(), self.current_index);

                self.finish().await?;
            }
        }


        Ok(())
    }

    pub async fn finish(&mut self) -> BuckyResult<()> {
        let writer = self.current.take();
        if writer.is_none() {
            return Ok(());
        }
        let mut writer = writer.unwrap();

        writer.finish().await?;
        self.current_index += 1;
        self.current = None;

        let file_path = writer.file_path().to_owned();
        let data_len = writer.total_bytes_added();
        drop(writer);

        let (hash, file_len) = cyfs_base::hash_file(&file_path).await.map_err(|e| {
            let msg = format!(
                "calc object pack file hash failed! file={}, {}",
                file_path.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let file_name = file_path.file_name().unwrap().to_string_lossy().to_string();
        let file_info = ObjectPackFileInfo {
            name: file_name,
            hash,
            file_len,
            data_len,
        };

        assert!(self
            .file_list
            .iter()
            .find(|item| item.name == file_info.name)
            .is_none());
        self.file_list.push(file_info);

        Ok(())
    }
}

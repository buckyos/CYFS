use super::super::pack::*;
use cyfs_base::*;

use std::path::PathBuf;

pub struct ObjectPackSerializeReader {
    format: ObjectPackFormat,

    root: PathBuf,
    file_list: Vec<ObjectPackFileInfo>,
    next_file_index: usize,

    current: Option<Box<dyn ObjectPackReader>>,
    next_zip_file_index: usize,

    crypto: Option<AesKey>,
}

impl ObjectPackSerializeReader {
    pub fn new(
        format: ObjectPackFormat,
        root: PathBuf,
        file_list: Vec<ObjectPackFileInfo>,
        crypto: Option<AesKey>,
    ) -> Self {
        Self {
            format,
            root,
            file_list,
            current: None,
            next_file_index: 0,
            next_zip_file_index: 0,
            crypto,
        }
    }

    async fn open(&mut self) -> BuckyResult<()> {
        assert!(self.current.is_none());
        assert!(self.next_file_index < self.file_list.len());

        let file_path = self.root.join(&self.file_list[self.next_file_index].name);

        info!(
            "will open pad file: index={}, file={}",
            self.next_file_index,
            file_path.display()
        );

        let mut reader = ObjectPackFactory::create_reader(self.format, file_path, self.crypto.clone());
        reader.open().await?;

        self.current = Some(reader);
        self.next_zip_file_index = 0;

        Ok(())
    }

    pub fn reset(&mut self) {
        let _ = self.current.take();

        self.next_file_index = 0;
        self.next_zip_file_index = 0;
    }

    pub async fn next_data(&mut self) -> BuckyResult<Option<(ObjectId, ObjectPackInnerFile)>> {
        let ret = loop {
            if self.current.is_none() {
                if self.next_file_index >= self.file_list.len() {
                    break None;
                }

                self.open().await?;
            }

            let ret = self.current.as_mut().unwrap().next_data().await?;
            if ret.is_none() {
                self.next_file_index += 1;
                let _ = self.current.take();
            } else {
                break ret;
            }
        };

        Ok(ret)
    }
}

struct FileItem {
    info: ObjectPackFileInfo,
    reader: Option<Box<dyn ObjectPackReader>>,
}

pub struct ObjectPackRandomReader {
    format: ObjectPackFormat,

    root: PathBuf,
    file_list: Vec<FileItem>,
    crypto: Option<AesKey>,
}

impl ObjectPackRandomReader {
    pub fn new(
        format: ObjectPackFormat,
        root: PathBuf,
        file_list: Vec<ObjectPackFileInfo>,
        crypto: Option<AesKey>,
    ) -> Self {
        Self {
            format,
            root,
            file_list: file_list
                .into_iter()
                .map(|info| FileItem { info, reader: None })
                .collect(),
            crypto,
        }
    }

    pub async fn open(&mut self) -> BuckyResult<()> {
        for item in self.file_list.iter_mut() {
            assert!(item.reader.is_none());

            let file_path = self.root.join(&item.info.name);

            info!("will open pack file: file={}", file_path.display());

            let mut reader =
                ObjectPackFactory::create_reader(self.format, file_path, self.crypto.clone());
            reader.open().await?;
            item.reader = Some(reader);
        }

        Ok(())
    }

    pub async fn get_data(
        &mut self,
        object_id: &ObjectId,
    ) -> BuckyResult<Option<ObjectPackInnerFile>> {
        for item in self.file_list.iter_mut() {
            let reader = item.reader.as_mut().unwrap();
            match reader.get_data(object_id).await {
                Ok(Some(info)) => {
                    return Ok(Some(info));
                }
                Ok(None) => continue,
                Err(e) => {
                    error!(
                        "get object from pack file failed! object={}, file={}, {}",
                        object_id, item.info.name, e
                    );
                    return Err(e);
                }
            }
        }

        error!(
            "get object from pack file but not found! object={}",
            object_id
        );
        Ok(None)
    }
}

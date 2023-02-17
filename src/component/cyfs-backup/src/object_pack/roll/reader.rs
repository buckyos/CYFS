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
}

impl ObjectPackSerializeReader {
    pub fn new( format: ObjectPackFormat, root: PathBuf, file_list: Vec<ObjectPackFileInfo>) -> Self {
        Self {
            format,
            root,
            file_list,
            current: None,
            next_file_index: 0,
            next_zip_file_index: 0,
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

        let mut reader = ObjectPackFactory::create_reader(self.format, file_path);
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

    pub async fn next_data(&mut self) -> BuckyResult<Option<(ObjectId, ObjectPackArchiveFile)>> {
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
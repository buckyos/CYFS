use super::super::pack::*;
use super::writer::ZipObjectPackWriter;
use cyfs_base::*;

use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

pub struct ZipObjectPackReader {
    path: PathBuf,
    reader: Option<zip::ZipArchive<File>>,
    next_file_index: usize,
}

impl ZipObjectPackReader {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            reader: None,
            next_file_index: 0,
        }
    }

    pub fn open(&mut self) -> BuckyResult<()> {
        let file = std::fs::OpenOptions::new()
            .read(true)
            .open(&self.path)
            .map_err(|e| {
                let msg = format!("open zip file failed! file={}, {}", self.path.display(), e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let reader = zip::ZipArchive::new(file).map_err(|e| {
            let msg = format!("open zip file failed! file={}, {}", self.path.display(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        assert!(self.reader.is_none());
        self.reader = Some(reader);

        Ok(())
    }

    pub fn close(&mut self) -> BuckyResult<()> {
        let current = self.reader.take();
        if let Some(reader) = current {
            info!("will close zip file: {}", self.path.display());
            drop(reader);
        }

        Ok(())
    }

    fn zip_file_to_reader(
        zip_file: &mut zip::read::ZipFile<'_>,
    ) -> BuckyResult<ObjectPackInnerFile> {
        let mut buffer = vec![];
        let bytes = zip_file.read_to_end(&mut buffer).map_err(|e| {
            let msg = format!("read zip file failed! file={}, {}", zip_file.name(), e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        if bytes as u64 != zip_file.size() {
            let msg = format!(
                "read zip file but length unmatch! file={}, len={}, got={}",
                zip_file.name(),
                zip_file.size(),
                bytes,
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }

        let meta = zip_file.extra_data();
        let meta = if !meta.is_empty() {
            if meta.len() > 4 {
                if &meta[..2] == super::writer::META_EXTRA_FIELD_ID.to_le_bytes() {
                    let len = u16::from_le_bytes(meta[2..4].try_into().unwrap());
                    let data = &meta[4..];
                    if len as usize == data.len() {
                        Some(data.to_owned())
                    } else {
                        error!("invalid zip extra data len! config={}, got={}", len, data.len());
                        None
                    }
                } else {
                    warn!("unknown zip extra field id: {:?}", &meta[..2]);
                    None
                }
            } else {
                error!("invalid zip extra field! len={}", meta.len());
                None
            }
            
        } else {
            None
        };

        let ret = ObjectPackInnerFile {
            data: ObjectPackInnerFileData::Buffer(buffer),
            meta,
        };

        Ok(ret)
    }

    pub fn get_data(&mut self, object_id: &ObjectId) -> BuckyResult<Option<ObjectPackInnerFile>> {
        let full_file_path = ZipObjectPackWriter::zip_inner_path(object_id);

        let reader = self.reader.as_mut().unwrap();

        let ret = match reader.by_name(&full_file_path) {
            Ok(mut file) => {
                // info!("file name: {}", file.name());

                let file = Self::zip_file_to_reader(&mut file)?;
                Ok(Some(file))
            }
            Err(e) => match e {
                zip::result::ZipError::FileNotFound => Ok(None),
                _ => {
                    todo!();
                }
            },
        };

        ret
    }

    pub fn reset(&mut self) {
        self.next_file_index = 0;
    }

    fn zip_inner_path_to_object_id(name: &str) -> BuckyResult<ObjectId> {
        let mut parts: Vec<&str> = name.split('/').rev().collect();
        if parts.len() != 2 {
            let msg = format!("invalid zip inner file name! name={}", name);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        if parts[1].ends_with("_") {
            parts[1] = parts[1].trim_end_matches('_');
        }

        let id36 = parts.join("");
        let object_id = ObjectId::from_base36(&id36).map_err(|e| {
            let msg = format!(
                "invalid zip inner file name! name={}, id36={}, {}",
                name, id36, e,
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        Ok(object_id)
    }

    pub fn next_data(&mut self) -> BuckyResult<Option<(ObjectId, ObjectPackInnerFile)>> {
        let reader = self.reader.as_mut().unwrap();

        let index = self.next_file_index;
        self.next_file_index += 1;
        if index >= reader.len() {
            return Ok(None);
        }

        let mut file = reader.by_index(index).unwrap();
        let object_id = Self::zip_inner_path_to_object_id(file.name())?;

        let file = Self::zip_file_to_reader(&mut file)?;
        Ok(Some((object_id, file)))
    }
}

#[async_trait::async_trait]
impl ObjectPackReader for ZipObjectPackReader {
    async fn open(&mut self) -> BuckyResult<()> {
        Self::open(self)
    }
    async fn close(&mut self) -> BuckyResult<()> {
        Self::close(self)
    }

    async fn get_data(&mut self, object_id: &ObjectId) -> BuckyResult<Option<ObjectPackInnerFile>> {
        Self::get_data(self, object_id)
    }

    async fn reset(&mut self) {
        Self::reset(self)
    }
    async fn next_data(&mut self) -> BuckyResult<Option<(ObjectId, ObjectPackInnerFile)>> {
        Self::next_data(self)
    }
}

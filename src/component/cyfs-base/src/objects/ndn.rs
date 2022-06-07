use crate::*;

#[derive(Clone)]
pub enum NDNId {
    DirId(DirId),
    FileId(FileId),
    DiffId(DiffId),
}

#[derive(Clone)]
pub enum NDNObject {
    Dir(Dir),
    File(File),
    Diff(Diff),
}

impl NDNObject {
    pub fn dir(&self) -> BuckyResult<&Dir> {
        match self {
            NDNObject::Dir(d) => Ok(d),
            _ => Err(BuckyError::new(
                BuckyErrorCode::InvalidParam,
                "type is not Dir",
            )),
        }
    }

    pub fn file(&self) -> BuckyResult<&File> {
        match self {
            NDNObject::File(d) => Ok(d),
            _ => Err(BuckyError::new(
                BuckyErrorCode::ParseError,
                "type is not File",
            )),
        }
    }

    pub fn diff(&self) -> BuckyResult<&Diff> {
        match self {
            NDNObject::Diff(d) => Ok(d),
            _ => Err(BuckyError::new(
                BuckyErrorCode::ParseError,
                "type is not Diff",
            )),
        }
    }

    pub fn object_id(&self) -> ObjectId {
        match self {
            NDNObject::Dir(d) => d.desc().dir_id().object_id().clone(),
            NDNObject::File(d) => d.desc().file_id().object_id().clone(),
            NDNObject::Diff(d) => d.desc().diff_id().object_id().clone(),
        }
    }
}

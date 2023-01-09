use super::def::*;
use crate::base::NDNDataRequestRange;
use crate::*;
use cyfs_base::*;

use async_std::io::Read;
use std::any::Any;
use std::fmt;
use std::fmt::Display;
use std::str::FromStr;
use std::sync::Arc;

pub type NDNInputRequestUserData = Arc<dyn Any + Sync + Send + 'static>;

#[derive(Clone, Debug)]
pub struct NDNInputRequestCommon {
    // 请求路径，可为空
    pub req_path: Option<String>,

    pub source: RequestSourceInfo,

    // api级别
    pub level: NDNAPILevel,

    // 需要处理数据的关联对象，主要用以chunk
    pub referer_object: Vec<NDNDataRefererObject>,

    // 用以处理默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,

    // input链的自定义数据
    pub user_data: Option<NDNInputRequestUserData>,
}

impl NDNInputRequestCommon {
    pub fn check_param_with_referer(&self, object_id: &ObjectId) -> BuckyResult<()> {
        match object_id.obj_type_code() {
            ObjectTypeCode::Chunk => {
                for item in &self.referer_object {
                    match item.object_id.obj_type_code() {
                        ObjectTypeCode::File => {
                            if !item.is_inner_path_empty() {
                                let msg = format!("ndn referer_object is file but inner_path is not empty! obj={}, referer={}", 
                                object_id, item);
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                            }
                        }
                        ObjectTypeCode::Dir => {}
                        ObjectTypeCode::ObjectMap => {
                            if item.is_inner_path_empty() {
                                let msg = format!("ndn referer_object is object_map but inner_path is empty! obj={}, referer={}", object_id, item);
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                            }
                        }
                        t @ _ => {
                            let msg = format!("unsupport ndn referer_object type for chunk! chunk={}, referer={}, type={:?}", 
                            object_id, item, t);
                            error!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                        }
                    }
                }
            }
            ObjectTypeCode::File => {
                for item in &self.referer_object {
                    match item.object_id.obj_type_code() {
                        ObjectTypeCode::Dir | ObjectTypeCode::ObjectMap => {
                            if item.is_inner_path_empty() {
                                let msg = format!("ndn referer_object is dir or object_map but inner_path is empty! obj={}, referer={}", 
                                object_id, item);
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                            }
                        }
                        t @ _ => {
                            let msg = format!("unsupport ndn referer_object type for file! obj={}, referer={}, type={:?}", 
                            object_id, item, t);
                            error!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                        }
                    }
                }
            }
            ObjectTypeCode::Dir | ObjectTypeCode::ObjectMap => {
                if self.referer_object.len() > 0 {
                    let msg = format!(
                        "ndn referer_object not support for dir/object_map! obj={}, type={:?}",
                        object_id,
                        object_id.obj_type_code()
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }
            }
            t @ _ => {
                let msg = format!(
                    "unsupport ndn object_id type! obj={}, type={:?}",
                    object_id, t,
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        }

        Ok(())
    }
}

impl fmt::Display for NDNInputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req_path: {:?}", self.req_path)?;
        write!(f, ", {}", self.source)?;
        write!(f, ", level: {}", self.level.to_string())?;

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target.to_string())?;
        }

        if !self.referer_object.is_empty() {
            write!(f, ", referer_object: {:?}", self.referer_object)?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum NDNDataType {
    Mem,
    SharedMem,
}

impl Display for NDNDataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Mem => "memory",
                Self::SharedMem => "shared_memory",
            }
        )
    }
}

impl FromStr for NDNDataType {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "memory" => Self::Mem,
            "shared_memory" => Self::SharedMem,
            v @ _ => {
                let msg = format!("unknown ndn data type: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        })
    }
}
/*
chunk_id
file_id
dir_id/inner_path
*/
#[derive(Clone)]
pub struct NDNGetDataInputRequest {
    pub common: NDNInputRequestCommon,

    // 目前只支持ChunkId/FileId/DirId
    pub object_id: ObjectId,

    pub data_type: NDNDataType,

    // request data range
    pub range: Option<NDNDataRequestRange>,

    // 对dir/objectmap有效
    pub inner_path: Option<String>,
}

impl fmt::Display for NDNGetDataInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;
        write!(f, ", data_type: {}", self.data_type)?;

        if let Some(range) = &self.range {
            write!(f, ", range: {}", range.to_display_string())?;
        }

        write!(f, ", inner_path: {:?}", self.inner_path)
    }
}

impl NDNGetDataInputRequest {
    pub fn check_valid(&self) -> BuckyResult<()> {
        self.common.check_param_with_referer(&self.object_id)
    }
}

pub struct NDNGetDataInputResponse {
    // chunk_id/file_id
    pub object_id: ObjectId,

    // file's owner
    pub owner_id: Option<ObjectId>,

    // 所属file的attr
    pub attr: Option<Attributes>,

    // resp ranges
    pub range: Option<NDNDataResponseRange>,

    pub length: u64,
    pub data: Box<dyn Read + Unpin + Send + Sync + 'static>,
}

impl fmt::Display for NDNGetDataInputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object_id: {}", self.object_id)?;

        if let Some(owner) = &self.owner_id {
            write!(f, ", owner: {}", owner)?;
        }

        if let Some(attr) = &self.attr {
            write!(f, ", attr: {:?}", attr)?;
        }

        if let Some(range) = &self.range {
            write!(f, ", range: {:?}", range)?;
        }

        write!(f, ", length: {}", self.length)
    }
}

// put_data，目前支持chunk
pub struct NDNPutDataInputRequest {
    pub common: NDNInputRequestCommon,

    pub object_id: ObjectId,
    pub data_type: NDNDataType,
    pub length: u64,
    pub data: Box<dyn Read + Unpin + Send + Sync + 'static>,
}

impl fmt::Display for NDNPutDataInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        write!(f, ", length: {:?}", self.length)
    }
}

impl NDNPutDataInputRequest {
    pub fn clone_without_data(&self) -> Self {
        Self {
            common: self.common.clone(),
            object_id: self.object_id.clone(),
            data_type: NDNDataType::Mem,
            length: self.length,
            data: Box::new(async_std::io::Cursor::new(vec![])),
        }
    }
}

pub struct NDNPutDataInputResponse {
    pub result: NDNPutDataResult,
}

impl fmt::Display for NDNPutDataInputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "result: {:?}", self.result)
    }
}

#[derive(Clone)]
pub struct NDNDeleteDataInputRequest {
    pub common: NDNInputRequestCommon,

    pub object_id: ObjectId,

    // 对dir_id有效
    pub inner_path: Option<String>,
}

impl fmt::Display for NDNDeleteDataInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        write!(f, ", inner_path: {:?}", self.inner_path)
    }
}

pub struct NDNDeleteDataInputResponse {
    pub object_id: ObjectId,
}

impl fmt::Display for NDNDeleteDataInputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object_id: {:?}", self.object_id)
    }
}

// query flags for the return value optional fields
pub const NDN_QUERY_FILE_REQUEST_FLAG_QUICK_HASN: u32 =
    cyfs_util::cache::NDC_FILE_REQUEST_FLAG_QUICK_HASN;
pub const NDN_QUERY_FILE_REQUEST_FLAG_REF_DIRS: u32 =
    cyfs_util::cache::NDC_FILE_REQUEST_FLAG_REF_DIRS;

#[derive(Debug, Clone)]
pub enum NDNQueryFileParam {
    File(ObjectId),
    Hash(HashValue),
    QuickHash(String),
    Chunk(ChunkId),
}

impl NDNQueryFileParam {
    pub fn as_str(&self) -> &str {
        match self {
            Self::File(_) => "file",
            Self::Hash(_) => "hash",
            Self::QuickHash(_) => "quick-hash",
            Self::Chunk(_) => "chunk",
        }
    }

    pub fn file_id(&self) -> Option<ObjectId> {
        match self {
            Self::File(id) => Some(id.to_owned()),
            _ => None,
        }
    }

    pub fn to_key_pair(&self) -> (&str, String) {
        let value = match self {
            Self::File(id) => id.to_string(),
            Self::Hash(hash) => hash.to_hex_string(),
            Self::QuickHash(hash) => hash.clone(),
            Self::Chunk(id) => id.to_string(),
        };

        (self.as_str(), value)
    }

    pub fn from_key_pair(t: &str, value: &str) -> BuckyResult<Self> {
        let ret = match t {
            "file" => {
                let value = ObjectId::from_str(value)?;
                Self::File(value)
            }
            "hash" => {
                let value = HashValue::from_str(value)?;
                Self::Hash(value)
            }
            "quick-hash" => Self::QuickHash(value.to_owned()),
            "chunk" => {
                let value = ChunkId::from_str(value)?;
                Self::Chunk(value)
            }
            _ => {
                let msg = format!("unknown NDNQueryFileParam: {}, {}", t, value);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        };

        Ok(ret)
    }
}

impl fmt::Display for NDNQueryFileParam {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (t, v) = self.to_key_pair();
        write!(f, "{}={}", t, v)
    }
}

#[derive(Clone)]
pub struct NDNQueryFileInputRequest {
    pub common: NDNInputRequestCommon,

    pub param: NDNQueryFileParam,
}

impl fmt::Display for NDNQueryFileInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", param: {}", self.param)
    }
}

pub struct NDNQueryFileInfo {
    pub file_id: FileId,

    pub hash: String,

    pub length: u64,

    pub flags: u32,

    pub owner: Option<ObjectId>,

    // 可选，关联的quickhash
    pub quick_hash: Option<Vec<String>>,

    // 可选，关联的dirs
    pub ref_dirs: Option<Vec<FileDirRef>>,
}

impl fmt::Display for NDNQueryFileInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "file_id: {}, hash={}, length={}, flags={}",
            self.file_id, self.hash, self.length, self.flags
        )?;

        if let Some(owner) = &self.owner {
            write!(f, ", owner={}", owner)?;
        }
        if let Some(quick_hash) = &self.quick_hash {
            write!(f, ", quick_hash={:?}", quick_hash)?;
        }
        if let Some(ref_dirs) = &self.ref_dirs {
            write!(f, ", ref_dirs={:?}", ref_dirs)?;
        }

        Ok(())
    }
}

pub struct NDNQueryFileInputResponse {
    pub list: Vec<NDNQueryFileInfo>,
}

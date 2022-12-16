use super::def::*;
use super::input_request::*;
use cyfs_base::*;
use crate::base::{NDNDataRequestRange, NDNDataResponseRange};

use async_std::io::Read;
use std::fmt;

#[derive(Debug, Clone)]
pub struct NDNOutputRequestCommon {
    // 请求路径，可为空
    pub req_path: Option<String>,

    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // api级别
    pub level: NDNAPILevel,

    // 用以处理默认行为
    pub target: Option<ObjectId>,

    // 需要处理数据的关联对象，主要用以chunk/file等
    pub referer_object: Vec<NDNDataRefererObject>,

    pub flags: u32,
}

impl NDNOutputRequestCommon {
    pub fn new(level: NDNAPILevel) -> Self {
        Self {
            req_path: None,
            dec_id: None,
            level,
            target: None,
            referer_object: vec![],
            flags: 0,
        }
    }
}

impl fmt::Display for NDNOutputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req_path: {:?}", self.req_path)?;
        write!(f, ", level: {:?}", self.level)?;

        if let Some(dec_id) = &self.dec_id {
            write!(f, ", dec_id: {}", dec_id)?;
        }

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target.to_string())?;
        }

        if self.referer_object.is_empty() {
            write!(f, ", referer_object: {:?}", self.referer_object)?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

// put requests
// 目前支持ChunkId
pub struct NDNPutDataOutputRequest {
    pub common: NDNOutputRequestCommon,

    pub object_id: ObjectId,

    pub length: u64,
    pub data: Box<dyn Read + Unpin + Send + Sync + 'static>,
}

impl fmt::Display for NDNPutDataOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        write!(f, ", length: {:?}", self.length)
    }
}

impl NDNPutDataOutputRequest {
    pub fn new(
        level: NDNAPILevel,
        object_id: ObjectId,
        length: u64,
        data: Box<dyn Read + Unpin + Send + Sync + 'static>,
    ) -> Self {
        Self {
            common: NDNOutputRequestCommon::new(level),
            object_id,
            length,
            data,
        }
    }

    pub fn new_with_buffer(level: NDNAPILevel, object_id: ObjectId, data: Vec<u8>) -> Self {
        let length = data.len() as u64;
        let data = async_std::io::Cursor::new(data);

        Self {
            common: NDNOutputRequestCommon::new(level),
            object_id,
            length,
            data: Box::new(data),
        }
    }

    pub fn new_ndc(
        object_id: ObjectId,
        length: u64,
        data: Box<dyn Read + Unpin + Send + Sync + 'static>,
    ) -> Self {
        Self::new(NDNAPILevel::NDC, object_id, length, data)
    }

    pub fn new_ndn(
        target: Option<DeviceId>,
        object_id: ObjectId,
        length: u64,
        data: Box<dyn Read + Unpin + Send + Sync + 'static>,
    ) -> Self {
        let mut ret = Self::new(NDNAPILevel::NDN, object_id, length, data);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(
        target: Option<ObjectId>,
        object_id: ObjectId,
        length: u64,
        data: Box<dyn Read + Unpin + Send + Sync + 'static>,
    ) -> Self {
        let mut ret = Self::new(NDNAPILevel::Router, object_id, length, data);
        ret.common.target = target;

        ret
    }

    pub fn new_router_with_buffer(
        target: Option<ObjectId>,
        object_id: ObjectId,
        data: Vec<u8>,
    ) -> Self {
        let mut ret = Self::new_with_buffer(NDNAPILevel::Router, object_id, data);
        ret.common.target = target;

        ret
    }
}

pub struct NDNPutDataOutputRequestWithBuffer {
    pub common: NDNOutputRequestCommon,

    pub object_id: ObjectId,
    pub data: Vec<u8>,
}

impl NDNPutDataOutputRequestWithBuffer {
    pub fn new(level: NDNAPILevel, object_id: ObjectId, data: Vec<u8>) -> Self {
        Self {
            common: NDNOutputRequestCommon::new(level),
            object_id,
            data,
        }
    }

    pub fn new_ndc(object_id: ObjectId, data: Vec<u8>) -> Self {
        Self::new(NDNAPILevel::NDC, object_id, data)
    }

    pub fn new_ndn(target: Option<DeviceId>, object_id: ObjectId, data: Vec<u8>) -> Self {
        let mut ret = Self::new(NDNAPILevel::NDN, object_id, data);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(target: Option<ObjectId>, object_id: ObjectId, data: Vec<u8>) -> Self {
        let mut ret = Self::new(NDNAPILevel::Router, object_id, data);
        ret.common.target = target;

        ret
    }
}

impl fmt::Display for NDNPutDataOutputRequestWithBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        write!(f, ", length: {}", self.data.len())
    }
}

pub struct NDNPutDataOutputResponse {
    pub result: NDNPutDataResult,
}

impl fmt::Display for NDNPutDataOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "result: {}", self.result.to_string())
    }
}

// get requests

/*
支持三种形式:
chunk_id
file_id
dir_id|object_map + inner_path
*/
#[derive(Clone)]
pub struct NDNGetDataOutputRequest {
    pub common: NDNOutputRequestCommon,

    // 目前只支持ChunkId/FileId/DirId
    pub object_id: ObjectId,

    pub range: Option<NDNDataRequestRange>,

    // 对dir_id有效
    pub inner_path: Option<String>,

    // trans data task group
    pub group: Option<String>,
}

impl NDNGetDataOutputRequest {
    pub fn new(level: NDNAPILevel, object_id: ObjectId, inner_path: Option<String>) -> Self {
        Self {
            common: NDNOutputRequestCommon::new(level),
            object_id,
            range: None,
            inner_path,
            group: None,
        }
    }

    pub fn new_ndc(object_id: ObjectId, inner_path: Option<String>) -> Self {
        Self::new(NDNAPILevel::NDC, object_id, inner_path)
    }

    pub fn new_ndn(
        target: Option<DeviceId>,
        object_id: ObjectId,
        inner_path: Option<String>,
    ) -> Self {
        let mut ret = Self::new(NDNAPILevel::NDN, object_id, inner_path);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(
        target: Option<ObjectId>,
        object_id: ObjectId,
        inner_path: Option<String>,
    ) -> Self {
        let mut ret = Self::new(NDNAPILevel::Router, object_id, inner_path);
        ret.common.target = target;

        ret
    }
}

impl fmt::Display for NDNGetDataOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        if let Some(range) = &self.range {
            write!(f, ", range: {}", range.to_display_string())?;
        }

        write!(f, ", inner_path: {:?}", self.inner_path)?;

        if let Some(group) = &self.group {
            write!(f, ", group: {}", group)?;
        }

        Ok(())
    }
}

pub struct NDNGetDataOutputResponse {
    // chunk_id/file_id
    pub object_id: ObjectId,

    // file's owner
    pub owner_id: Option<ObjectId>,

    // 所属file的attr
    pub attr: Option<Attributes>,

    // resp ranges
    pub range: Option<NDNDataResponseRange>,

    // content
    pub length: u64,
    pub data: Box<dyn Read + Unpin + Send + Sync + 'static>,
}

impl fmt::Display for NDNGetDataOutputResponse {
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

#[derive(Clone)]
pub struct NDNDeleteDataOutputRequest {
    pub common: NDNOutputRequestCommon,

    pub object_id: ObjectId,

    // 对dir_id有效
    pub inner_path: Option<String>,
}

impl NDNDeleteDataOutputRequest {
    pub fn new(level: NDNAPILevel, object_id: ObjectId, inner_path: Option<String>) -> Self {
        Self {
            common: NDNOutputRequestCommon::new(level),
            object_id,
            inner_path,
        }
    }

    pub fn new_ndc(object_id: ObjectId, inner_path: Option<String>) -> Self {
        Self::new(NDNAPILevel::NDC, object_id, inner_path)
    }

    pub fn new_ndn(
        target: Option<DeviceId>,
        object_id: ObjectId,
        inner_path: Option<String>,
    ) -> Self {
        let mut ret = Self::new(NDNAPILevel::NDN, object_id, inner_path);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(
        target: Option<ObjectId>,
        object_id: ObjectId,
        inner_path: Option<String>,
    ) -> Self {
        let mut ret = Self::new(NDNAPILevel::Router, object_id, inner_path);
        ret.common.target = target;

        ret
    }
}

impl fmt::Display for NDNDeleteDataOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        write!(f, ", inner_path: {:?}", self.inner_path)
    }
}

pub struct NDNDeleteDataOutputResponse {
    pub object_id: ObjectId,
}

impl fmt::Display for NDNDeleteDataOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object_id: {}", self.object_id)
    }
}


#[derive(Clone)]
pub struct NDNQueryFileOutputRequest {
    pub common: NDNOutputRequestCommon,

    pub param: NDNQueryFileParam,
}

impl NDNQueryFileOutputRequest {
    pub fn new(level: NDNAPILevel, param: NDNQueryFileParam) -> Self {
        Self {
            common: NDNOutputRequestCommon::new(level),
            param,
        }
    }

    pub fn new_ndc(param: NDNQueryFileParam) -> Self {
        Self::new(NDNAPILevel::NDC, param)
    }

    pub fn new_ndn(
        target: Option<DeviceId>,
        param: NDNQueryFileParam,
    ) -> Self {
        let mut ret = Self::new(NDNAPILevel::NDN, param);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(
        target: Option<ObjectId>,
        param: NDNQueryFileParam,
    ) -> Self {
        let mut ret = Self::new(NDNAPILevel::Router, param);
        ret.common.target = target;

        ret
    }
}

impl fmt::Display for NDNQueryFileOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", param: {:?}", self.param)
    }
}

pub type NDNQueryFileOutputResponse = NDNQueryFileInputResponse;
use super::def::*;
use crate::*;
use cyfs_base::*;

use std::fmt;

#[derive(Clone)]
pub struct NONOutputRequestCommon {
    // 请求路径，可为空
    pub req_path: Option<String>,

    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // api级别
    pub level: NONAPILevel,

    // 用以处理默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl NONOutputRequestCommon {
    pub fn new(level: NONAPILevel) -> Self {
        Self {
            req_path: None,
            dec_id: None,
            level,
            target: None,
            flags: 0,
        }
    }
}

impl fmt::Display for NONOutputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req_path: {:?}", self.req_path)?;
        write!(f, ", level: {:?}", self.level)?;

        if let Some(dec_id) = &self.dec_id {
            write!(f, ", dec_id: {}", dec_id)?;
        }

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target.to_string())?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

// put requests
#[derive(Clone)]
pub struct NONPutObjectOutputRequest {
    pub common: NONOutputRequestCommon,

    pub object: NONObjectInfo,
}

impl NONPutObjectOutputRequest {
    pub fn new(level: NONAPILevel, object_id: ObjectId, object_raw: Vec<u8>) -> Self {
        Self {
            common: NONOutputRequestCommon::new(level),
            object: NONObjectInfo::new(object_id, object_raw, None),
        }
    }

    pub fn new_noc(object_id: ObjectId, object_raw: Vec<u8>) -> Self {
        Self::new(NONAPILevel::NOC, object_id, object_raw)
    }

    pub fn new_non(target: Option<DeviceId>, object_id: ObjectId, object_raw: Vec<u8>) -> Self {
        let mut ret = Self::new(NONAPILevel::NON, object_id, object_raw);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(target: Option<ObjectId>, object_id: ObjectId, object_raw: Vec<u8>) -> Self {
        let mut ret = Self::new(NONAPILevel::Router, object_id, object_raw);
        ret.common.target = target;

        ret
    }
}

impl fmt::Display for NONPutObjectOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object: {}", self.object)
    }
}

#[derive(Clone)]
pub struct NONPutObjectOutputResponse {
    pub result: NONPutObjectResult,
    pub object_update_time: Option<u64>,
    pub object_expires_time: Option<u64>,
}

impl fmt::Display for NONPutObjectOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "result: {:?}", self.result)?;
        write!(f, ", object_update_time: {:?}", self.object_update_time)?;
        write!(f, ", object_expires_time: {:?}", self.object_expires_time)
    }
}

// get requests
#[derive(Clone)]
pub struct NONGetObjectOutputRequest {
    pub common: NONOutputRequestCommon,

    pub object_id: ObjectId,

    // inner_path在dir情况下适用
    pub inner_path: Option<String>,
}

impl NONGetObjectOutputRequest {
    pub fn new(level: NONAPILevel, object_id: ObjectId, inner_path: Option<String>) -> Self {
        Self {
            common: NONOutputRequestCommon::new(level),
            object_id,
            inner_path,
        }
    }

    pub fn new_noc(object_id: ObjectId, inner_path: Option<String>) -> Self {
        Self::new(NONAPILevel::NOC, object_id, inner_path)
    }

    pub fn new_non(
        target: Option<DeviceId>,
        object_id: ObjectId,
        inner_path: Option<String>,
    ) -> Self {
        let mut ret = Self::new(NONAPILevel::NON, object_id, inner_path);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(
        target: Option<ObjectId>,
        object_id: ObjectId,
        inner_path: Option<String>,
    ) -> Self {
        let mut ret = Self::new(NONAPILevel::Router, object_id, inner_path);
        ret.common.target = target;

        ret
    }
}


impl fmt::Display for NONGetObjectOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        write!(f, ", inner_path: {:?}", self.inner_path)
    }
}

#[derive(Clone)]
pub struct NONGetObjectOutputResponse {
    pub object_update_time: Option<u64>,
    pub object_expires_time: Option<u64>,

    pub object: NONObjectInfo,

    // 对file有效
    pub attr: Option<Attributes>,
}

impl fmt::Display for NONGetObjectOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object: {}", self.object)?;
        write!(f, ", object_update_time: {:?}", self.object_update_time)?;
        write!(f, ", object_expires_time: {:?}", self.object_expires_time)?;

        if let Some(attr) = &self.attr {
            write!(f, ", attr: {:?}", attr)?;
        }

        Ok(())
    }
}

// POST请求
#[derive(Clone)]
pub struct NONPostObjectOutputRequest {
    pub common: NONOutputRequestCommon,

    pub object: NONObjectInfo,
}

impl NONPostObjectOutputRequest {
    pub fn new(level: NONAPILevel, object_id: ObjectId, object_raw: Vec<u8>) -> Self {
        Self {
            common: NONOutputRequestCommon::new(level),
            object: NONObjectInfo::new(object_id, object_raw, None),
        }
    }

    pub fn new_noc(object_id: ObjectId, object_raw: Vec<u8>) -> Self {
        Self::new(NONAPILevel::NOC, object_id, object_raw)
    }

    pub fn new_non(target: Option<DeviceId>, object_id: ObjectId, object_raw: Vec<u8>) -> Self {
        let mut ret = Self::new(NONAPILevel::NON, object_id, object_raw);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(target: Option<ObjectId>, object_id: ObjectId, object_raw: Vec<u8>) -> Self {
        let mut ret = Self::new(NONAPILevel::Router, object_id, object_raw);
        ret.common.target = target;

        ret
    }
}

impl fmt::Display for NONPostObjectOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object: {}", self.object)
    }
}

#[derive(Clone)]
pub struct NONPostObjectOutputResponse {
    pub object: Option<NONObjectInfo>,
}

impl fmt::Display for NONPostObjectOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.object {
            Some(object) => {
                write!(f, "object: {}", object,)
            }
            None => {
                write!(f, "none",)
            }
        }
    }
}


// select
#[derive(Clone)]
pub struct NONSelectObjectOutputRequest {
    pub common: NONOutputRequestCommon,

    pub filter: SelectFilter,
    pub opt: Option<SelectOption>,
}

impl NONSelectObjectOutputRequest {
    pub fn new(level: NONAPILevel, filter: SelectFilter, opt: Option<SelectOption>) -> Self {
        Self {
            common: NONOutputRequestCommon::new(level),
            filter,
            opt,
        }
    }

    pub fn new_noc(filter: SelectFilter, opt: Option<SelectOption>) -> Self {
        Self::new(NONAPILevel::NOC, filter, opt)
    }

    pub fn new_non(
        target: Option<DeviceId>,
        filter: SelectFilter,
        opt: Option<SelectOption>,
    ) -> Self {
        let mut ret = Self::new(NONAPILevel::NON, filter, opt);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(
        target: Option<ObjectId>,
        filter: SelectFilter,
        opt: Option<SelectOption>,
    ) -> Self {
        let mut ret = Self::new(NONAPILevel::Router, filter, opt);
        ret.common.target = target;

        ret
    }
}

impl fmt::Display for NONSelectObjectOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", filter: {}", self.filter)?;
        write!(f, ", opt: {:?}", self.opt)
    }
}


#[derive(Clone)]
pub struct NONSelectObjectOutputResponse {
    pub objects: Vec<SelectResponseObjectInfo>,
}

impl fmt::Display for NONSelectObjectOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "select count: {}, list=[", self.objects.len())?;

        for item in &self.objects {
            write!(f, "{{ {} }}, ", item)?;
        }

        write!(f, "]")?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct NONDeleteObjectOutputRequest {
    pub common: NONOutputRequestCommon,

    pub object_id: ObjectId,

    pub inner_path: Option<String>,
}

impl NONDeleteObjectOutputRequest {
    pub fn new(level: NONAPILevel, object_id: ObjectId, inner_path: Option<String>) -> Self {
        Self {
            common: NONOutputRequestCommon::new(level),
            object_id,
            inner_path,
        }
    }

    pub fn new_noc(object_id: ObjectId, inner_path: Option<String>) -> Self {
        Self::new(NONAPILevel::NOC, object_id, inner_path)
    }

    pub fn new_non(
        target: Option<DeviceId>,
        object_id: ObjectId,
        inner_path: Option<String>,
    ) -> Self {
        let mut ret = Self::new(NONAPILevel::NON, object_id, inner_path);
        ret.common.target = target.map(|v| v.into());

        ret
    }

    pub fn new_router(
        target: Option<ObjectId>,
        object_id: ObjectId,
        inner_path: Option<String>,
    ) -> Self {
        let mut ret = Self::new(NONAPILevel::Router, object_id, inner_path);
        ret.common.target = target;

        ret
    }
}

impl fmt::Display for NONDeleteObjectOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        if let Some(inner_path) = &self.inner_path {
            write!(f, ", inner_path: {}", inner_path)?;
        }

        Ok(())
    }
}


#[derive(Clone)]
pub struct NONDeleteObjectOutputResponse {
    pub object: Option<NONObjectInfo>,
}

impl fmt::Display for NONDeleteObjectOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object: {:?}", self.object)?;
    
        Ok(())
    }
}

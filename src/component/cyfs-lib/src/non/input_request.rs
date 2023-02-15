use crate::*;
use cyfs_base::*;

use std::fmt;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct NONInputRequestCommon {
    // 请求路径，可为空
    pub req_path: Option<String>,

    // the request source info in bundle
    pub source: RequestSourceInfo,

    // api级别
    pub level: NONAPILevel,

    // 用以处理默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl fmt::Display for NONInputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req_path: {:?}", self.req_path)?;

        write!(f, ", {}", self.source)?;
        write!(f, ", level: {}", self.level.to_string())?;

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target.to_string())?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

/*
/object_id
/dir_id|object_map/inner_path
*/
#[derive(Clone, Debug)]
pub struct NONGetObjectInputRequest {
    pub common: NONInputRequestCommon,

    pub object_id: ObjectId,

    // object_id在dir情况下适用
    pub inner_path: Option<String>,
}

impl fmt::Display for NONGetObjectInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        write!(f, ", inner_path: {:?}", self.inner_path)
    }
}

impl NONGetObjectInputRequest {
    pub fn is_with_inner_path_relation(&self) -> bool {
        match self.object_id.obj_type_code() {
            ObjectTypeCode::ObjectMap | ObjectTypeCode::Dir => {
                self.inner_path.is_some()
            }
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct NONGetObjectInputResponse {
    pub object_update_time: Option<u64>,
    pub object_expires_time: Option<u64>,

    pub object: NONObjectInfo,

    // 对file有效
    pub attr: Option<Attributes>,
}

impl NONGetObjectInputResponse {
    pub fn new(
        object_id: ObjectId,
        object_raw: Vec<u8>,
        object: Option<Arc<AnyNamedObject>>,
    ) -> Self {
        let object = NONObjectInfo::new(object_id, object_raw, object);
        Self::new_with_object(object)
    }

    pub fn new_with_object(object: NONObjectInfo) -> Self {
        Self {
            object,
            object_expires_time: None,
            object_update_time: None,
            attr: None,
        }
    }

    pub fn init_times(&mut self) -> BuckyResult<()> {
        let t = self.object.get_update_time()?;
        if t > 0 {
            self.object_update_time = Some(t);
        }

        let t = self.object.get_expired_time()?;
        self.object_expires_time = t;
        Ok(())
    }
}

impl fmt::Display for NONGetObjectInputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, ", object: {}", self.object)?;
        write!(f, ", object_update_time: {:?}", self.object_update_time)?;
        write!(f, ", object_expires_time: {:?}", self.object_expires_time)?;

        if let Some(attr) = &self.attr {
            write!(f, ", attr: {:?}", attr)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct NONPutObjectInputRequest {
    pub common: NONInputRequestCommon,

    pub object: NONObjectInfo,
    pub access: Option<AccessString>,
}

impl fmt::Display for NONPutObjectInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object: {}", self.object)?;
        if let Some(access) = &self.access {
            write!(f, ", access: {}", access.to_string())?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct NONPutObjectInputResponse {
    pub result: NONPutObjectResult,
    pub object_update_time: Option<u64>,
    pub object_expires_time: Option<u64>,
}

impl Default for NONPutObjectInputResponse {
    fn default() -> Self {
        Self::new(NONPutObjectResult::Accept)
    }
}

impl NONPutObjectInputResponse {
    pub fn new(result: NONPutObjectResult) -> Self {
        Self {
            result,
            object_update_time: None,
            object_expires_time: None,
        }
    }
}

impl fmt::Display for NONPutObjectInputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "result: {:?}", self.result)?;
        write!(f, ", object_update_time: {:?}", self.object_update_time)?;
        write!(f, ", object_expires_time: {:?}", self.object_expires_time)
    }
}

// post_object请求
#[derive(Debug, Clone)]
pub struct NONPostObjectInputRequest {
    pub common: NONInputRequestCommon,

    pub object: NONObjectInfo,
}

impl fmt::Display for NONPostObjectInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object: {}", self.object)
    }
}

#[derive(Debug)]
pub struct NONPostObjectInputResponse {
    pub object: Option<NONObjectInfo>,
}

impl fmt::Display for NONPostObjectInputResponse {
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

// select object
#[derive(Debug, Clone)]
pub struct NONSelectObjectInputRequest {
    pub common: NONInputRequestCommon,

    pub filter: SelectFilter,
    pub opt: Option<SelectOption>,
}

impl fmt::Display for NONSelectObjectInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", filter: {}", self.filter)?;
        write!(f, ", opt: {:?}", self.opt)
    }
}

#[derive(Debug)]
pub struct NONSelectObjectInputResponse {
    pub objects: Vec<SelectResponseObjectInfo>,
}

impl fmt::Display for NONSelectObjectInputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "select count: {}, list=[", self.objects.len())?;

        for item in &self.objects {
            write!(f, "{{ {} }}, ", item)?;
        }

        write!(f, "]")?;

        Ok(())
    }
}

// delete object
#[derive(Debug, Clone)]
pub struct NONDeleteObjectInputRequest {
    pub common: NONInputRequestCommon,

    pub object_id: ObjectId,

    pub inner_path: Option<String>,
}

impl fmt::Display for NONDeleteObjectInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object_id: {}", self.object_id)?;

        if let Some(inner_path) = &self.inner_path {
            write!(f, ", inner_path: {}", inner_path)?;
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct NONDeleteObjectInputResponse {
    pub object: Option<NONObjectInfo>,
}

impl fmt::Display for NONDeleteObjectInputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(item) = &self.object {
            write!(f, "object: {}", item)?;
        }
        Ok(())
    }
}

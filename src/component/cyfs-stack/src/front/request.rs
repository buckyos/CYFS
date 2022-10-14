use super::def::*;
use cyfs_base::*;
use cyfs_lib::*;

#[derive(Clone, Debug)]
pub struct FrontORequest {
    pub source: RequestSourceInfo,

    pub target: Vec<ObjectId>,

    pub object_id: ObjectId,
    pub inner_path: Option<String>,
    pub range: Option<NDNDataRequestRange>,

    pub mode: FrontRequestGetMode,
    pub format: FrontRequestObjectFormat,

    pub flags: u32,
}

pub struct FrontOResponse {
    pub object: Option<NONGetObjectInputResponse>,
    pub data: Option<NDNGetDataInputResponse>,
}

#[derive(Clone, Debug)]
pub struct FrontRRequest {
    // 来源信息
    pub source: RequestSourceInfo,

    pub category: GlobalStateCategory,

    pub target: Option<ObjectId>,

    pub target_dec_id: Option<ObjectId>,

    pub action: GlobalStateAccessorAction,
    pub inner_path: Option<String>,
    pub range: Option<NDNDataRequestRange>,

    pub page_index: Option<u32>,
    pub page_size: Option<u32>,

    pub mode: FrontRequestGetMode,
    pub flags: u32,
}

pub struct FrontRResponse {
    pub object: Option<NONGetObjectInputResponse>,
    pub root: ObjectId,
    pub revision: u64,

    pub data: Option<NDNGetDataInputResponse>,

    // for list action
    pub list: Option<Vec<ObjectMapContentItem>>,
}

pub struct FrontNDNRequest {
    // 来源信息
    pub source: RequestSourceInfo,

    pub target: Vec<ObjectId>,

    pub object: NONObjectInfo,
    pub range: Option<NDNDataRequestRange>,

    pub flags: u32,
}

impl FrontNDNRequest {
    pub fn new_o_chunk(req: FrontORequest) -> Self {
        assert_eq!(req.object_id.obj_type_code(), ObjectTypeCode::Chunk);

        FrontNDNRequest {
            source: req.source,
            target: req.target,
            object: NONObjectInfo::new(req.object_id, vec![], None),
            range: req.range,
            flags: req.flags,
        }
    }

    pub fn new_o_file(req: FrontORequest, object: NONObjectInfo) -> Self {
        assert_eq!(object.object_id.obj_type_code(), ObjectTypeCode::File);

        FrontNDNRequest {
            source: req.source,
            target: req.target,
            object,
            range: req.range,
            flags: req.flags,
        }
    }

    pub fn new_r_resp(req: FrontRRequest, object: NONObjectInfo) -> Self {
        let target = match req.target {
            Some(target) => vec![target],
            None => vec![],
        };

        FrontNDNRequest {
            source: req.source,
            target,
            object,
            range: req.range,
            flags: req.flags,
        }
    }
}

#[derive(Debug, Clone)]
pub enum FrontARequestDec {
    DecID(ObjectId),
    Name(String),
}

impl FrontARequestDec {
    pub fn as_dec_id(&self) -> Option<&ObjectId> {
        match self {
            Self::DecID(ref id) => Some(id),
            Self::Name(_) => None,
        }
    }

    pub fn as_name(&self) -> Option<&str> {
        match self {
            Self::Name(ref name) => Some(name.as_str()),
            Self::DecID(_) => None,
        }
    }
}

#[derive(Debug)]
pub enum FrontARequestVersion {
    Version(String),
    DirID(ObjectId),
    Current,
}

#[derive(Debug)]
pub struct FrontARequestWeb {
    pub version: FrontARequestVersion,
    pub inner_path: Option<String>,
}

#[derive(Debug)]
pub enum FrontARequestGoal {
    Web(FrontARequestWeb),
    LocalStatus,
}

#[derive(Debug)]
pub struct FrontARequest {
    // 来源信息
    pub source: RequestSourceInfo,

    pub target: Option<ObjectId>,

    pub dec: FrontARequestDec,
    pub goal: FrontARequestGoal,

    pub mode: FrontRequestGetMode,
    pub format: FrontRequestObjectFormat,

    pub origin_url: http_types::Url,
    pub flags: u32,
}

pub enum FrontAResponse {
    Response(FrontOResponse),
    Redirect(String),
}

use super::def::*;
use cyfs_base::*;
use cyfs_lib::*;

#[derive(Clone, Debug)]
pub struct FrontORequest {
    pub source: RequestSourceInfo,

    pub req_path: Option<String>,
    pub target: Vec<ObjectId>,

    pub object_id: ObjectId,
    pub inner_path: Option<String>,
    pub range: Option<NDNDataRequestRange>,

    // for ndn requests
    pub referer_objects: Vec<NDNDataRefererObject>,
    pub context: Option<String>,
    pub group: Option<String>,

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

    pub context: Option<String>,
    pub group: Option<String>,

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

    pub req_path: Option<String>,
    pub referer_objects: Vec<NDNDataRefererObject>,

    pub context: Option<String>,
    pub group: Option<String>,

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
            req_path: req.req_path,
            referer_objects: req.referer_objects,
            context: req.context,
            group: req.group,
            flags: req.flags,
        }
    }

    pub fn new_o_file(mut req: FrontORequest, object: NONObjectInfo) -> Self {
        assert_eq!(object.object_id.obj_type_code(), ObjectTypeCode::File);

        let referer_objects = if req.object_id != object.object_id {
            let referer_object = NDNDataRefererObject {
                target: None,
                object_id: req.object_id,
                inner_path: req.inner_path,
            };
            let mut referer_objects= vec![referer_object];
            if req.referer_objects.len() > 0 {
                referer_objects.append(&mut req.referer_objects)
            }
            referer_objects
        } else {
            req.referer_objects
        };

        FrontNDNRequest {
            source: req.source,
            target: req.target,
            object,
            range: req.range,
            req_path: req.req_path,
            referer_objects,
            context: req.context,
            group: req.group,
            flags: req.flags,
        }
    }

    pub fn new_r_resp(req: FrontRRequest, state_resp: &RootStateAccessorGetObjectByPathInputResponse) -> Self {
        let target = match req.target {
            Some(target) => vec![target],
            None => vec![],
        };

        let mut req_path = RequestGlobalStatePath::new(req.target_dec_id.clone(), req.inner_path.clone());
        req_path.set_root(state_resp.root.clone());

        FrontNDNRequest {
            source: req.source,
            target,
            object: state_resp.object.object.clone(),
            range: req.range,
            req_path: Some(req_path.format_string()),
            referer_objects: vec![],
            context: req.context,
            group: req.group,
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

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
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

    pub referer_objects: Vec<NDNDataRefererObject>,
    pub context: Option<String>,
    pub group: Option<String>,

    pub flags: u32,
}

pub enum FrontAResponse {
    Response(FrontOResponse),
    Redirect(String),
}

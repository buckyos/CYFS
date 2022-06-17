use super::def::*;
use cyfs_base::*;
use cyfs_lib::*;

#[derive(Clone, Debug)]
pub struct FrontORequest {
    pub protocol: NONProtocol,
    pub source: DeviceId,

    pub target: Vec<ObjectId>,

    pub dec_id: Option<ObjectId>,

    pub object_id: ObjectId,
    pub inner_path: Option<String>,

    pub mode: FrontRequestGetMode,

    pub flags: u32,
}

pub struct FrontOResponse {
    pub object: Option<NONGetObjectInputResponse>,
    pub data: Option<NDNGetDataInputResponse>,
}

#[derive(Clone, Debug)]
pub struct FrontRRequest {
    pub protocol: NONProtocol,
    pub source: DeviceId,

    pub category: GlobalStateCategory,

    pub target: Option<ObjectId>,

    pub dec_id: Option<ObjectId>,
    pub inner_path: Option<String>,

    pub mode: FrontRequestGetMode,

    pub flags: u32,
}

pub struct FrontRResponse {
    pub object: Option<NONGetObjectInputResponse>,
    pub root: ObjectId,
    pub revision: u64,

    pub data: Option<NDNGetDataInputResponse>,
}

pub struct FrontNDNRequest {
    pub protocol: NONProtocol,
    pub source: DeviceId,

    pub target: Vec<ObjectId>,
    pub dec_id: Option<ObjectId>,

    pub object: NONObjectInfo,

    pub flags: u32,
}

impl FrontNDNRequest {
    pub fn new_o_chunk(req: FrontORequest) -> Self {
        assert_eq!(req.object_id.obj_type_code(), ObjectTypeCode::Chunk);

        FrontNDNRequest {
            protocol: req.protocol,
            source: req.source,

            target: req.target,
            dec_id: req.dec_id,

            object: NONObjectInfo::new(req.object_id, vec![], None),
            flags: req.flags,
        }
    }

    pub fn new_o_file(req: FrontORequest, object: NONObjectInfo) -> Self {
        assert_eq!(object.object_id.obj_type_code(), ObjectTypeCode::File);

        FrontNDNRequest {
            protocol: req.protocol,
            source: req.source,

            target: req.target,
            dec_id: req.dec_id,

            object,
            flags: req.flags,
        }
    }

    pub fn new_r_resp(req: FrontRRequest, object: NONObjectInfo) -> Self {
        let target = match req.target {
            Some(target) => vec![target],
            None => vec![],
        };

        FrontNDNRequest {
            protocol: req.protocol,
            source: req.source,

            target,
            dec_id: req.dec_id,

            object,
            flags: req.flags,
        }
    }
}

pub struct FrontARequest {}

pub struct FrontAResponse {}

use super::output_request::*;
use crate::base::*;
use cyfs_base::*;
use std::path::PathBuf;

pub struct UtilInputRequestCommon {
    // 请求路径，可为空
    pub req_path: Option<String>,

    pub source: RequestSourceInfo,

    // 用以默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

// get device
pub struct UtilGetDeviceInputRequest {
    pub common: UtilInputRequestCommon,
}

pub type UtilGetDeviceInputResponse = UtilGetDeviceOutputResponse;

// get zone
pub struct UtilGetZoneInputRequest {
    pub common: UtilInputRequestCommon,

    pub object_id: Option<ObjectId>,
    pub object_raw: Option<Vec<u8>>,
}

pub type UtilGetZoneInputResponse = UtilGetZoneOutputResponse;

// resolve_ood
pub struct UtilResolveOODInputRequest {
    pub common: UtilInputRequestCommon,

    pub object_id: ObjectId,
    pub owner_id: Option<ObjectId>,
}

pub type UtilResolveOODInputResponse = UtilResolveOODOutputResponse;

// get_ood_status
pub struct UtilGetOODStatusInputRequest {
    pub common: UtilInputRequestCommon,
}

pub type UtilGetOODStatusInputResponse = UtilGetOODStatusOutputResponse;

// get_noc_stat
pub struct UtilGetNOCInfoInputRequest {
    pub common: UtilInputRequestCommon,
}

pub type UtilGetNOCInfoInputResponse = UtilGetNOCInfoOutputResponse;

// get_device_static_info
pub struct UtilGetDeviceStaticInfoInputRequest {
    pub common: UtilInputRequestCommon,
}

pub type UtilGetDeviceStaticInfoInputResponse = UtilGetDeviceStaticInfoOutputResponse;

// get_network_access_info
pub struct UtilGetNetworkAccessInfoInputRequest {
    pub common: UtilInputRequestCommon,
}

pub type UtilGetNetworkAccessInfoInputResponse = UtilGetNetworkAccessInfoOutputResponse;

// get_system_info
pub struct UtilGetSystemInfoInputRequest {
    pub common: UtilInputRequestCommon,
}

pub type UtilGetSystemInfoInputResponse = UtilGetSystemInfoOutputResponse;

// get_version_info
pub struct UtilGetVersionInfoInputRequest {
    pub common: UtilInputRequestCommon,
}

pub type UtilGetVersionInfoInputResponse = UtilGetVersionInfoOutputResponse;

pub struct UtilBuildFileInputRequest {
    pub common: UtilInputRequestCommon,
    pub local_path: PathBuf,
    pub owner: ObjectId,
    pub chunk_size: u32,
    pub access: Option<AccessString>,
}

pub type UtilBuildFileInputResponse = UtilBuildFileOutputResponse;

pub struct UtilBuildDirFromObjectMapInputRequest {
    pub common: UtilInputRequestCommon,
    pub object_map_id: ObjectId,
    pub dir_type: BuildDirType,
}

pub type UtilBuildDirFromObjectMapInputResponse = UtilBuildDirFromObjectMapOutputResponse;

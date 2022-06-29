use super::output_request::*;
use crate::*;
use cyfs_base::*;

use std::fmt;

#[derive(Clone, Debug)]
pub struct RootStateInputRequestCommon {
    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // 来源设备和协议
    pub source: DeviceId,
    pub protocol: NONProtocol,

    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl fmt::Display for RootStateInputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(dec_id) = &self.dec_id {
            write!(f, "dec_id: {}", dec_id)?;
        }
        write!(f, ", source: {}", self.source.to_string())?;
        write!(f, ", protocol: {}", self.protocol.to_string())?;

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target)?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

// get_current_root

#[derive(Clone)]
pub struct RootStateGetCurrentRootInputRequest {
    pub common: RootStateInputRequestCommon,

    pub root_type: RootStateRootType,
}

impl fmt::Display for RootStateGetCurrentRootInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}, root_type={:?}", self.common, self.root_type)
    }
}

pub type RootStateGetCurrentRootInputResponse = RootStateGetCurrentRootOutputResponse;

// create_op_env
#[derive(Clone)]
pub struct RootStateCreateOpEnvInputRequest {
    pub common: RootStateInputRequestCommon,

    pub op_env_type: ObjectMapOpEnvType,
}

impl fmt::Display for RootStateCreateOpEnvInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", op_env_type: {}", self.op_env_type.to_string())
    }
}

pub type RootStateCreateOpEnvInputResponse = RootStateCreateOpEnvOutputResponse;

#[derive(Clone, Debug)]
pub struct OpEnvInputRequestCommon {
    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // 来源设备和协议
    pub source: DeviceId,
    pub protocol: NONProtocol,

    pub target: Option<ObjectId>,

    pub flags: u32,

    // 所属session id
    pub sid: u64,
}

impl fmt::Display for OpEnvInputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(dec_id) = &self.dec_id {
            write!(f, ", dec_id: {}", dec_id)?;
        }
        write!(f, ", source: {}", self.source.to_string())?;
        write!(f, ", protocol: {}", self.protocol.to_string())?;

        write!(f, ", flags: {}", self.flags)?;

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target)?;
        }

        write!(f, ", sid: {}", self.sid)?;

        Ok(())
    }
}

/// single_op_env methods
// load
pub struct OpEnvLoadInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub target: ObjectId,
}

impl fmt::Display for OpEnvLoadInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", target: {}", self.target)
    }
}

// load_by_path
pub struct OpEnvLoadByPathInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub path: String,
}

impl fmt::Display for OpEnvLoadByPathInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", path: {}", self.path)
    }
}

// create_new
pub struct OpEnvCreateNewInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub path: Option<String>,
    pub key: Option<String>,

    pub content_type: ObjectMapSimpleContentType,
}

impl fmt::Display for OpEnvCreateNewInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        if let Some(path) = &self.path {
            write!(f, ", path: {}", path)?;
        }
        if let Some(key) = &self.key {
            write!(f, ", key: {}", key)?;
        }
        write!(f, ", content_type: {:?}", self.content_type)
    }
}

// lock
pub struct OpEnvLockInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub path_list: Vec<String>,
    pub duration_in_millsecs: u64,
    pub try_lock: bool,
}

impl fmt::Display for OpEnvLockInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", path_list: {:?}", self.path_list)?;
        write!(f, ", duration_in_millsecs: {}", self.duration_in_millsecs)?;
        write!(f, ", try_lock: {}", self.try_lock)
    }
}

// commit
pub struct OpEnvCommitInputRequest {
    pub common: OpEnvInputRequestCommon,
    pub op_type: Option<OpEnvCommitOpType>,
}

impl fmt::Display for OpEnvCommitInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}, op_type: {:?}", self.common, self.op_type)
    }
}

pub type OpEnvCommitInputResponse = OpEnvCommitOutputResponse;

// abort
pub struct OpEnvAbortInputRequest {
    pub common: OpEnvInputRequestCommon,
}

impl fmt::Display for OpEnvAbortInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)
    }
}

// metadata
pub struct OpEnvMetadataInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub path: Option<String>,
}

impl fmt::Display for OpEnvMetadataInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}, path: {:?}", self.common, self.path)
    }
}

pub type OpEnvMetadataInputResponse = OpEnvMetadataOutputResponse;

// get_by_key
#[derive(Clone)]
pub struct OpEnvGetByKeyInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub path: Option<String>,
    pub key: String,
}

impl fmt::Display for OpEnvGetByKeyInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        if let Some(path) = &self.path {
            write!(f, ", path: {}", path)?;
        }

        write!(f, ", key: {}", self.key)
    }
}

pub type OpEnvGetByKeyInputResponse = OpEnvGetByKeyOutputResponse;

// insert_with_key
#[derive(Clone)]
pub struct OpEnvInsertWithKeyInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub path: Option<String>,
    pub key: String,
    pub value: ObjectId,
}

impl fmt::Display for OpEnvInsertWithKeyInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        if let Some(path) = &self.path {
            write!(f, ", path: {}", path)?;
        }

        write!(f, ", key: {}", self.key)?;
        write!(f, ", value: {}", self.value)
    }
}

// set_with_key
#[derive(Clone)]
pub struct OpEnvSetWithKeyInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub path: Option<String>,
    pub key: String,
    pub value: ObjectId,
    pub prev_value: Option<ObjectId>,
    pub auto_insert: bool,
}

impl fmt::Display for OpEnvSetWithKeyInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        if let Some(path) = &self.path {
            write!(f, ", path: {}", path)?;
        }

        write!(f, ", key: {}", self.key)?;
        write!(f, ", value: {}", self.value)?;
        write!(f, ", prev_value: {:?}", self.prev_value)?;
        write!(f, ", auto_insert: {}", self.auto_insert)
    }
}

pub type OpEnvSetWithKeyInputResponse = OpEnvSetWithKeyOutputResponse;

// remove_with_key
#[derive(Clone)]
pub struct OpEnvRemoveWithKeyInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub path: Option<String>,
    pub key: String,
    pub prev_value: Option<ObjectId>,
}

impl fmt::Display for OpEnvRemoveWithKeyInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        if let Some(path) = &self.path {
            write!(f, ", path: {}", path)?;
        }

        write!(f, ", key: {}", self.key)?;
        write!(f, ", prev_value: {:?}", self.prev_value)
    }
}

pub type OpEnvRemoveWithKeyInputResponse = OpEnvRemoveWithKeyOutputResponse;

// set模式通用的request
pub struct OpEnvSetInputRequest {
    pub common: OpEnvInputRequestCommon,

    pub path: Option<String>,
    pub value: ObjectId,
}

impl fmt::Display for OpEnvSetInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        if let Some(path) = &self.path {
            write!(f, ", path: {}", path)?;
        }

        write!(f, ", value: {}", self.value)
    }
}

pub type OpEnvSetInputResponse = OpEnvSetOutputResponse;

// contains
pub type OpEnvContainsInputRequest = OpEnvSetInputRequest;
pub type OpEnvContainsInputResponse = OpEnvSetInputResponse;

// insert
pub type OpEnvInsertInputRequest = OpEnvSetInputRequest;
pub type OpEnvInsertInputResponse = OpEnvSetInputResponse;

// remove
pub type OpEnvRemoveInputRequest = OpEnvSetInputRequest;
pub type OpEnvRemoveInputResponse = OpEnvSetInputResponse;

// 迭代器next
pub struct OpEnvNextInputRequest {
    pub common: OpEnvInputRequestCommon,

    // 步进的元素个数
    pub step: u32,
}

impl fmt::Display for OpEnvNextInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        write!(f, ", step: {}", self.step)
    }
}

pub type OpEnvNextInputResponse = OpEnvNextOutputResponse;

//////////////////////////
/// root-state access requests

// get_object_by_path
#[derive(Clone)]
pub struct RootStateAccessGetObjectByPathInputRequest {
    pub common: RootStateInputRequestCommon,

    pub inner_path: String,
}

impl fmt::Display for RootStateAccessGetObjectByPathInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", inner_path: {}", self.inner_path)
    }
}

pub struct RootStateAccessGetObjectByPathInputResponse {
    pub object: NONGetObjectInputResponse,
    pub root: ObjectId,
    pub revision: u64,
}

// list
pub struct RootStateAccessListInputRequest {
    pub common: RootStateInputRequestCommon,

    pub inner_path: String,

    // read elements by page
    pub page_index: Option<u32>,
    pub page_size: Option<u32>,
}

impl fmt::Display for RootStateAccessListInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        write!(
            f,
            ", inner_path={}, page_index: {:?}, page_size: {:?}",
            self.inner_path, self.page_index, self.page_size
        )
    }
}

pub type RootStateAccessListInputResponse = RootStateAccessListOutputResponse;

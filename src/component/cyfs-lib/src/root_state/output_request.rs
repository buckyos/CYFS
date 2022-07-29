use crate::*;
use cyfs_base::*;

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Debug)]
pub struct RootStateOutputRequestCommon {
    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // 用以默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl RootStateOutputRequestCommon {
    pub fn new() -> Self {
        Self {
            dec_id: None,
            target: None,
            flags: 0,
        }
    }
}

impl fmt::Display for RootStateOutputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(dec_id) = &self.dec_id {
            write!(f, "dec_id: {}", dec_id)?;
        }

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target)?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

// get_current_root
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RootStateRootType {
    Global,
    Dec,
}

impl ToString for RootStateRootType {
    fn to_string(&self) -> String {
        (match *self {
            Self::Global => "global",
            Self::Dec => "dec",
        })
        .to_owned()
    }
}

impl FromStr for RootStateRootType {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "global" => Self::Global,
            "dec" => Self::Dec,

            v @ _ => {
                let msg = format!("unknown root_type value: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Clone)]
pub struct RootStateGetCurrentRootOutputRequest {
    pub common: RootStateOutputRequestCommon,

    pub root_type: RootStateRootType,
}

impl RootStateGetCurrentRootOutputRequest {
    pub fn new_global() -> Self {
        Self {
            common: RootStateOutputRequestCommon::new(),
            root_type: RootStateRootType::Global,
        }
    }
    pub fn new_dec() -> Self {
        Self {
            common: RootStateOutputRequestCommon::new(),
            root_type: RootStateRootType::Dec,
        }
    }
}

#[derive(Debug)]
pub struct RootStateGetCurrentRootOutputResponse {
    pub root: ObjectId,
    pub revision: u64,
    pub dec_root: Option<ObjectId>,
}

// create_op_env
#[derive(Clone)]
pub struct RootStateCreateOpEnvOutputRequest {
    pub common: RootStateOutputRequestCommon,

    pub op_env_type: ObjectMapOpEnvType,
}

impl RootStateCreateOpEnvOutputRequest {
    pub fn new(op_env_type: ObjectMapOpEnvType) -> Self {
        Self {
            common: RootStateOutputRequestCommon::new(),
            op_env_type,
        }
    }
}

pub struct RootStateCreateOpEnvOutputResponse {
    pub sid: u64,
}

#[derive(Clone, Debug)]
pub struct OpEnvOutputRequestCommon {
    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // 用以默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,

    // 所属session id
    pub sid: u64,
}

impl OpEnvOutputRequestCommon {
    pub fn new_empty() -> Self {
        Self {
            dec_id: None,
            target: None,
            flags: 0,
            sid: 0,
        }
    }
}

impl fmt::Display for OpEnvOutputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sid: {}", self.sid)?;

        if let Some(dec_id) = &self.dec_id {
            write!(f, ", dec_id: {}", dec_id)?;
        }

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target)?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

pub struct OpEnvNoParamOutputRequest {
    pub common: OpEnvOutputRequestCommon,
}

impl OpEnvNoParamOutputRequest {
    pub fn new() -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
        }
    }
}

/// single_op_env methods
// load
pub struct OpEnvLoadOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    pub target: ObjectId,
}

impl OpEnvLoadOutputRequest {
    pub fn new(target: ObjectId) -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            target,
        }
    }
}

// load_by_path
pub struct OpEnvLoadByPathOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    pub path: String,
}

impl OpEnvLoadByPathOutputRequest {
    pub fn new(path: String) -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path,
        }
    }
}

// create_new
pub struct OpEnvCreateNewOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    pub path: Option<String>,
    pub key: Option<String>,
    pub content_type: ObjectMapSimpleContentType,
}

impl OpEnvCreateNewOutputRequest {
    pub fn new(content_type: ObjectMapSimpleContentType) -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key: None,
            content_type,
        }
    }

    pub fn new_with_full_path(full_path: impl Into<String>, content_type: ObjectMapSimpleContentType) -> Self {
        let full_path = full_path.into();
        assert!(full_path.len() > 0);

        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key: Some(full_path),
            content_type,
        }
    }

    pub fn new_with_path_and_key(path: impl Into<String>, key: impl Into<String>, content_type: ObjectMapSimpleContentType) -> Self {
        let path = path.into();
        let key = key.into();
        assert!(OpEnvPathHelper::check_valid(&path, &key));

        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: Some(path),
            key: Some(key),
            content_type,
        }
    }
}

// lock
pub struct OpEnvLockOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    pub path_list: Vec<String>,
    pub duration_in_millsecs: u64,
    pub try_lock: bool,
}

impl OpEnvLockOutputRequest {
    pub fn new(path_list: Vec<String>, duration_in_millsecs: u64) -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path_list,
            duration_in_millsecs,
            try_lock: false,
        }
    }

    pub fn new_try(path_list: Vec<String>, duration_in_millsecs: u64) -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path_list,
            duration_in_millsecs,
            try_lock: true,
        }
    }
}

// get_current_root
pub type OpEnvGetCurrentRootOutputRequest = OpEnvNoParamOutputRequest;
pub type OpEnvGetCurrentRootOutputResponse = OpEnvCommitOutputResponse;

#[derive(Clone, Debug)]
pub enum OpEnvCommitOpType {
    Commit,
    Update,
}

impl OpEnvCommitOpType {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Commit => "commit",
            Self::Update => "update",
        }
    }
}

impl ToString for OpEnvCommitOpType {
    fn to_string(&self) -> String {
        self.as_str().to_owned()
    }
}

impl FromStr for OpEnvCommitOpType {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "commit" => Self::Commit,
            "update" => Self::Update,

            v @ _ => {
                let msg = format!("unknown OpEnvCommitOpType value: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

impl Default for OpEnvCommitOpType {
    fn default() -> Self {
        Self::Commit
    }
}

// commit
pub struct OpEnvCommitOutputRequest {
    pub common: OpEnvOutputRequestCommon,
    pub op_type: Option<OpEnvCommitOpType>,
}

impl OpEnvCommitOutputRequest {
    pub fn new() -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            op_type: None,
        }
    }

    pub fn new_update() -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            op_type: Some(OpEnvCommitOpType::Update),
        }
    }
}

pub struct OpEnvCommitOutputResponse {
    pub root: ObjectId,
    pub revision: u64,

    pub dec_root: ObjectId,
}

// abort
pub type OpEnvAbortOutputRequest = OpEnvNoParamOutputRequest;


pub struct OpEnvPathHelper {}

impl OpEnvPathHelper {
    pub fn check_key_valid(key: &str) -> bool {
        if key.is_empty() || key.find("/").is_some() {
            return false;
        }

        true
    }

    pub fn check_valid(path: &str, key: &str) -> bool {
        if path.is_empty() || !Self::check_key_valid(key) {
            return false;
        }

        true
    }

    pub fn join(path: &str, key: &str) -> String {
        if path.ends_with("/") {
            format!("{}{}", path, key)
        } else {
            format!("{}/{}", path, key)
        }
    }
}

// metadata
pub struct OpEnvMetadataOutputRequest {
    pub common: OpEnvOutputRequestCommon,
    pub path: Option<String>,
}

impl OpEnvMetadataOutputRequest {
    pub fn new(path: Option<String>) -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path,
        }
    }
}

#[derive(Debug)]
pub struct OpEnvMetadataOutputResponse {
    pub content_mode: ObjectMapContentMode,
    pub content_type: ObjectMapSimpleContentType,
    pub count: u64,
    pub size: u64,
    pub depth: u8,
}

// get_by_key
#[derive(Clone)]
pub struct OpEnvGetByKeyOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    pub path: Option<String>,
    pub key: String,
}

impl fmt::Display for OpEnvGetByKeyOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        if let Some(path) = &self.path {
            write!(f, ", path: {}", path)?;
        }

        write!(f, ", key: {}", self.key)
    }
}

impl OpEnvGetByKeyOutputRequest {
    pub fn new_path_and_key(path: impl Into<String>, key: impl Into<String>) -> Self {
        let path = path.into();
        let key = key.into();

        assert!(OpEnvPathHelper::check_valid(&path, &key));

        let req = OpEnvGetByKeyOutputRequest {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: Some(path),
            key,
        };

        req
    }

    pub fn new_full_path(full_path: impl Into<String>) -> Self {
        let full_path = full_path.into();
        assert!(full_path.len() > 0);

        let req = OpEnvGetByKeyOutputRequest {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key: full_path,
        };

        req
    }

    pub fn new_key(key: impl Into<String>) -> Self {
        let key = key.into();
        assert!(OpEnvPathHelper::check_key_valid(&key));

        let req = OpEnvGetByKeyOutputRequest {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key,
        };

        req
    }
}

#[derive(Debug)]
pub struct OpEnvGetByKeyOutputResponse {
    pub value: Option<ObjectId>,
}

// insert_with_key
#[derive(Clone)]
pub struct OpEnvInsertWithKeyOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    pub path: Option<String>,
    pub key: String,
    pub value: ObjectId,
}

impl OpEnvInsertWithKeyOutputRequest {
    pub fn new_path_and_key_value(
        path: impl Into<String>,
        key: impl Into<String>,
        value: ObjectId,
    ) -> Self {
        let path = path.into();
        let key = key.into();
        assert!(OpEnvPathHelper::check_valid(&path, &key));

        let req = Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: Some(path),
            key,
            value,
        };

        req
    }

    pub fn new_full_path_and_value(full_path: impl Into<String>, value: ObjectId) -> Self {
        let full_path = full_path.into();
        assert!(full_path.len() > 0);

        let req = Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key: full_path,
            value,
        };

        req
    }

    pub fn new_key_value(key: impl Into<String>, value: ObjectId) -> Self {
        let key = key.into();
        assert!(OpEnvPathHelper::check_key_valid(&key));

        let req = Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key,
            value,
        };

        req
    }
}

// set_with_key
#[derive(Clone)]
pub struct OpEnvSetWithKeyOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    pub path: Option<String>,
    pub key: String,
    pub value: ObjectId,
    pub prev_value: Option<ObjectId>,
    pub auto_insert: bool,
}

impl OpEnvSetWithKeyOutputRequest {
    pub fn new_path_and_key_value(
        path: impl Into<String>,
        key: impl Into<String>,
        value: ObjectId,
        prev_value: Option<ObjectId>,
        auto_insert: bool,
    ) -> Self {
        let path = path.into();
        let key = key.into();
        assert!(OpEnvPathHelper::check_valid(&path, &key));

        let req = Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: Some(path),
            key,
            value,
            prev_value,
            auto_insert,
        };

        req
    }

    pub fn new_full_path_and_value(
        full_path: impl Into<String>,
        value: ObjectId,
        prev_value: Option<ObjectId>,
        auto_insert: bool,
    ) -> Self {
        let full_path = full_path.into();
        assert!(full_path.len() > 0);

        let req = Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key: full_path,
            value,
            prev_value,
            auto_insert,
        };

        req
    }

    pub fn new_key_value(
        key: impl Into<String>,
        value: ObjectId,
        prev_value: Option<ObjectId>,
        auto_insert: bool,
    ) -> Self {
        let key = key.into();
        assert!(OpEnvPathHelper::check_key_valid(&key));

        let req = Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key,
            value,
            prev_value,
            auto_insert,
        };

        req
    }
}

#[derive(Clone)]
pub struct OpEnvSetWithKeyOutputResponse {
    pub prev_value: Option<ObjectId>,
}

// remove_with_key
#[derive(Clone)]
pub struct OpEnvRemoveWithKeyOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    pub path: Option<String>,
    pub key: String,
    pub prev_value: Option<ObjectId>,
}

impl OpEnvRemoveWithKeyOutputRequest {
    pub fn new_path_and_key(
        path: impl Into<String>,
        key: impl Into<String>,
        prev_value: Option<ObjectId>,
    ) -> Self {
        let path = path.into();
        let key = key.into();
        assert!(OpEnvPathHelper::check_valid(&path, &key));

        let req = Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: Some(path),
            key,
            prev_value,
        };

        req
    }

    pub fn new_full_path(full_path: impl Into<String>, prev_value: Option<ObjectId>) -> Self {
        let full_path = full_path.into();
        assert!(full_path.len() > 0);

        let req = OpEnvRemoveWithKeyOutputRequest {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key: full_path,
            prev_value,
        };

        req
    }

    pub fn new_key(key: impl Into<String>, prev_value: Option<ObjectId>) -> Self {
        let key = key.into();
        assert!(key.len() > 0);

        let req = OpEnvRemoveWithKeyOutputRequest {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            key,
            prev_value,
        };

        req
    }
}

#[derive(Clone)]
pub struct OpEnvRemoveWithKeyOutputResponse {
    pub value: Option<ObjectId>,
}

// set模式通用的request
pub struct OpEnvSetOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    pub path: Option<String>,
    pub value: ObjectId,
}

impl OpEnvSetOutputRequest {
    pub fn new_path(path: impl Into<String>, value: ObjectId) -> Self {
        let path = path.into();
        assert!(path.len() > 0);

        let req = OpEnvSetOutputRequest {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: Some(path),
            value,
        };

        req
    }

    pub fn new(value: ObjectId) -> Self {
        let req = OpEnvContainsOutputRequest {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
            value,
        };

        req
    }
}

pub struct OpEnvSetOutputResponse {
    pub result: bool,
}

// contains
pub type OpEnvContainsOutputRequest = OpEnvSetOutputRequest;
pub type OpEnvContainsOutputResponse = OpEnvSetOutputResponse;

// insert
pub type OpEnvInsertOutputRequest = OpEnvSetOutputRequest;
pub type OpEnvInsertOutputResponse = OpEnvSetOutputResponse;

// remove
pub type OpEnvRemoveOutputRequest = OpEnvSetOutputRequest;
pub type OpEnvRemoveOutputResponse = OpEnvSetOutputResponse;

// 迭代器

// next
pub struct OpEnvNextOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    // 步进的元素个数
    pub step: u32,
}

impl OpEnvNextOutputRequest {
    pub fn new(step: u32) -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            step,
        }
    }
}

pub struct OpEnvNextOutputResponse {
    pub list: Vec<ObjectMapContentItem>,
}

// reset
pub type OpEnvResetOutputRequest = OpEnvNoParamOutputRequest;


// list
pub struct OpEnvListOutputRequest {
    pub common: OpEnvOutputRequestCommon,

    // for path-env
    pub path: Option<String>,
}

impl OpEnvListOutputRequest {
    pub fn new() -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: None,
        }
    }

    pub fn new_path(path: impl Into<String>) -> Self {
        Self {
            common: OpEnvOutputRequestCommon::new_empty(),
            path: Some(path.into()),
        }
    }
}

pub type OpEnvListOutputResponse = OpEnvNextOutputResponse;


//////////////////////////
/// root-state access requests

// get_object_by_path
#[derive(Clone)]
pub struct RootStateAccessGetObjectByPathOutputRequest {
    pub common: RootStateOutputRequestCommon,

    pub inner_path: String,
}

impl fmt::Display for RootStateAccessGetObjectByPathOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", inner_path: {}", self.inner_path)
    }
}

pub struct RootStateAccessGetObjectByPathOutputResponse {
    pub object: NONGetObjectOutputResponse,
    pub root: ObjectId,
    pub revision: u64,
}

impl fmt::Display for RootStateAccessGetObjectByPathOutputResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.object.fmt(f)?;
        write!(f, ", root: {}, revision: {}", self.root, self.revision)
    }
}

// list
pub struct RootStateAccessListOutputRequest {
    pub common: RootStateOutputRequestCommon,

    pub inner_path: String,

    // read elements by page
    pub page_index: Option<u32>,
    pub page_size: Option<u32>,
}

impl fmt::Display for RootStateAccessListOutputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        write!(
            f,
            ", inner_path={}, page_index: {:?}, page_size: {:?}",
            self.inner_path, self.page_index, self.page_size
        )
    }
}

pub struct RootStateAccessListOutputResponse {
    pub list: Vec<ObjectMapContentItem>,

    pub root: ObjectId,
    pub revision: u64,
}

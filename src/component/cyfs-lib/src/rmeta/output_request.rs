
use super::def::*;
use cyfs_base::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetaOutputRequestCommon {
    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // 目标DEC
    pub target_dec_id: Option<ObjectId>,

    // 用以默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl MetaOutputRequestCommon {
    pub fn new() -> Self {
        Self {
            dec_id: None,
            target_dec_id: None,
            target: None,
            flags: 0,
        }
    }
}

impl std::fmt::Display for MetaOutputRequestCommon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(dec_id) = &self.dec_id {
            write!(f, "dec_id: {}", dec_id)?;
        }
        if let Some(target_dec_id) = &self.target_dec_id {
            write!(f, ", target_dec_id: {}", target_dec_id)?;
        }

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target)?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaAddAccessOutputRequest {
    pub common: MetaOutputRequestCommon,

    pub item: GlobalStatePathAccessItem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaAddAccessOutputResponse {
    pub updated: bool,
}

pub type GlobalStateMetaRemoveAccessOutputRequest = GlobalStateMetaAddAccessOutputRequest;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaRemoveAccessOutputResponse {
    pub item: Option<GlobalStatePathAccessItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaClearAccessOutputRequest {
    pub common: MetaOutputRequestCommon,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaClearAccessOutputResponse {
    pub count: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaAddLinkOutputRequest {
    pub common: MetaOutputRequestCommon,

    pub source: String,
    pub target: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaAddLinkOutputResponse {
    pub updated: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaRemoveLinkOutputRequest {
    pub common: MetaOutputRequestCommon,

    pub source: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaRemoveLinkOutputResponse {
    pub item: Option<GlobalStatePathLinkItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaClearLinkOutputRequest {
    pub common: MetaOutputRequestCommon,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaClearLinkOutputResponse {
    pub count: u32,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaAddObjectMetaOutputRequest {
    pub common: MetaOutputRequestCommon,

    pub item: GlobalStateObjectMetaItem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaAddObjectMetaOutputResponse {
    pub updated: bool,
}

pub type GlobalStateMetaRemoveObjectMetaOutputRequest = GlobalStateMetaAddObjectMetaOutputRequest;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaRemoveObjectMetaOutputResponse {
    pub item: Option<GlobalStateObjectMetaItem>,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaClearObjectMetaOutputRequest {
    pub common: MetaOutputRequestCommon,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaClearObjectMetaOutputResponse {
    pub count: u32,
}

// path config
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaAddPathConfigOutputRequest {
    pub common: MetaOutputRequestCommon,

    pub item: GlobalStatePathConfigItem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaAddPathConfigOutputResponse {
    pub updated: bool,
}

pub type GlobalStateMetaRemovePathConfigOutputRequest = GlobalStateMetaAddPathConfigOutputRequest;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaRemovePathConfigOutputResponse {
    pub item: Option<GlobalStatePathConfigItem>,
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaClearPathConfigOutputRequest {
    pub common: MetaOutputRequestCommon,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GlobalStateMetaClearPathConfigOutputResponse {
    pub count: u32,
}
use super::def::*;
use super::output_request::*;
use crate::base::*;
use cyfs_base::*;

#[derive(Clone, Debug)]
pub struct MetaInputRequestCommon {
    // 来源
    pub source: RequestSourceInfo,

    // 目标DEC，如果为空，则默认等价于source-dec-id
    pub target_dec_id: Option<ObjectId>,

    // 用以默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl std::fmt::Display for MetaInputRequestCommon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)?;

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

#[derive(Clone, Debug)]
pub struct GlobalStateMetaAddAccessInputRequest {
    pub common: MetaInputRequestCommon,

    pub item: GlobalStatePathAccessItem,
}

pub type GlobalStateMetaAddAccessInputResponse = GlobalStateMetaAddAccessOutputResponse;

pub type GlobalStateMetaRemoveAccessInputRequest = GlobalStateMetaAddAccessInputRequest;
pub type GlobalStateMetaRemoveAccessInputResponse = GlobalStateMetaRemoveAccessOutputResponse;

#[derive(Clone, Debug)]
pub struct GlobalStateMetaClearAccessInputRequest {
    pub common: MetaInputRequestCommon,
}

pub type GlobalStateMetaClearAccessInputResponse = GlobalStateMetaClearAccessOutputResponse;

#[derive(Clone, Debug)]
pub struct GlobalStateMetaAddLinkInputRequest {
    pub common: MetaInputRequestCommon,

    pub source: String,
    pub target: String,
}

pub type GlobalStateMetaAddLinkInputResponse = GlobalStateMetaAddLinkOutputResponse;

#[derive(Clone, Debug)]
pub struct GlobalStateMetaRemoveLinkInputRequest {
    pub common: MetaInputRequestCommon,

    pub source: String,
}

pub type GlobalStateMetaRemoveLinkInputResponse = GlobalStateMetaRemoveLinkOutputResponse;

#[derive(Clone, Debug)]
pub struct GlobalStateMetaClearLinkInputRequest {
    pub common: MetaInputRequestCommon,
}

pub type GlobalStateMetaClearLinkInputResponse = GlobalStateMetaClearLinkOutputResponse;

// object meta 
#[derive(Clone, Debug)]
pub struct GlobalStateMetaAddObjectMetaInputRequest {
    pub common: MetaInputRequestCommon,

    pub item: GlobalStateObjectMetaItem,
}

pub type GlobalStateMetaAddObjectMetaInputResponse = GlobalStateMetaAddObjectMetaOutputResponse;

pub type GlobalStateMetaRemoveObjectMetaInputRequest = GlobalStateMetaAddObjectMetaInputRequest;
pub type GlobalStateMetaRemoveObjectMetaInputResponse = GlobalStateMetaRemoveObjectMetaOutputResponse;

#[derive(Clone, Debug)]
pub struct GlobalStateMetaClearObjectMetaInputRequest {
    pub common: MetaInputRequestCommon,
}

pub type GlobalStateMetaClearObjectMetaInputResponse = GlobalStateMetaClearObjectMetaOutputResponse;

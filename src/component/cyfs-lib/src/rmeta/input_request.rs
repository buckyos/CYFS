use super::def::*;
use super::output_request::*;
use crate::*;
use cyfs_base::*;

#[derive(Clone, Debug)]
pub struct MetaInputRequestCommon {
    // 来源DEC
    pub dec_id: Option<ObjectId>,

    // 来源设备和协议
    pub source: DeviceId,
    pub protocol: NONProtocol,

    // 用以默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl std::fmt::Display for MetaInputRequestCommon {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

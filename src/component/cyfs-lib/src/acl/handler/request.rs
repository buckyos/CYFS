use super::super::{AclAction};
use crate::base::*;
use cyfs_base::*;

#[derive(Debug, Clone)]
pub struct AclHandlerRequest {
    // The owner dec
    pub dec_id: ObjectId,

    pub source: RequestSourceInfo,

    pub req_path: String,
    pub req_query_string: Option<String>,

    // The required permissions
    pub permissions: AccessPermissions,
}

impl std::fmt::Display for AclHandlerRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "dec_id: {}", self.dec_id)?;
        write!(f, ", source: {}", self.source)?;
        write!(f, ", req_path: {}", self.req_path)?;
        if let Some(v) = &self.req_query_string {
            write!(f, ", req_query_string: {}", v)?;
        }
        write!(f, ", permissions: {}", self.permissions)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AclHandlerResponse {
    pub action: AclAction,
}

impl std::fmt::Display for AclHandlerResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "action: {:?}", self.action)
    }
}

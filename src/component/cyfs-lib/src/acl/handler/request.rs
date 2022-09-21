use super::super::{AclAccess, AclAction};
use crate::ndn::*;
use crate::non::NONSlimObjectInfo;
use cyfs_base::*;

pub struct AclHandlerRequest {
    // 来源协议
    pub protocol: RequestProtocol,

    // 动作
    pub action: AclAction,

    // source/target
    pub device_id: DeviceId,

    // 操作对象
    pub object: Option<NONSlimObjectInfo>,
    pub inner_path: Option<String>,

    // 所属dec
    pub dec_id: String,

    // 请求的path
    pub req_path: Option<String>,

    // 引用对象
    pub referer_object: Option<Vec<NDNDataRefererObject>>,
}

impl std::fmt::Display for AclHandlerRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "protocol: {:?}", self.protocol)?;
        write!(f, ", action: {:?}", self.action)?;
        write!(f, ", device: {}", self.device_id)?;

        if let Some(object) = &self.object {
            write!(f, ", object: {:?}", object)?;
        }

        if let Some(inner_path) = &self.inner_path {
            write!(f, ", inner_path: {}", inner_path)?;
        }

        write!(f, ", dec: {}", self.dec_id)?;
        write!(f, ", req_path: {:?}", self.req_path)?;
        if let Some(referer_object) = &self.referer_object {
            write!(f, ", referer_object: {:?}", referer_object)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AclHandlerResponse {
    pub access: AclAccess,
}

impl std::fmt::Display for AclHandlerResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "access: {:?}", self.access)
    }
}

use super::access::AclAccessEx;
use super::group::*;
use super::relation::AclRelationManager;
use super::request::*;
use super::res::*;
use crate::router_handler::RouterHandlersManager;
use cyfs_base::*;
use cyfs_lib::*;

use std::str::FromStr;

pub(crate) struct AclItem {
    id: String,

    action: AclAction,
    res: AclResource,
    group: AclGroup,
    access: AclAccessEx,
}

impl AclItem {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub async fn try_match(&self, req: &dyn AclRequest) -> BuckyResult<Option<AclAccess>> {
        trace!("will match req: id={}, req={}", self.id, req);

        // 首先匹配action
        if !self.action.is_match(req.action()) {
            trace!("action not match! id={}", self.id);
            return Ok(None);
        }

        // 路径有一个匹配上就可以
        let mut ret = false;
        for item in req.resource().await.iter() {
            if self.res.is_match(item.as_str()) {
                trace!("res is match! id={}, res={}", self.id, item);
                ret = true;
                break;
            }
        }
        if !ret {
            return Ok(None);
        }

        // 最后匹配group
        let ret = self.group.is_match(req).await?;
        if !ret {
            trace!("group not match! id={}", self.id);
            return Ok(None);
        }

        trace!("acl matched! id={}", self.id);

        // 计算access
        let access = self.access.get_access(req).await?;

        Ok(Some(access))
    }

    pub fn load(
        id: String,
        router_handlers: &RouterHandlersManager,
        relation_manager: &AclRelationManager,
        table: toml::value::Table,
    ) -> BuckyResult<Self> {
        let mut action = AclAction::default();
        let mut res = AclResource::Any;
        let mut group = AclGroup::default();
        let mut access = None;

        for (k, v) in table.into_iter() {
            match k.as_str() {
                "action" => match v.as_str() {
                    Some(s) => {
                        action = AclAction::parse(s)?;
                    }
                    None => {
                        let msg = format!("acl action node invalid type: {:?}", v);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                },
                "res" => match v.as_str() {
                    Some(s) => {
                        res = AclResource::from_str(s)?;
                    }
                    None => {
                        let msg = format!("acl res node invalid type: {:?}", v);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                },
                "group" => {
                    group = AclGroup::load(relation_manager, v)?;
                }
                "access" => match v.as_str() {
                    Some(s) => {
                        access = Some(AclAccessEx::load(router_handlers, &id, s)?);
                    }
                    None => {
                        let msg = format!("acl access node invalid type: {:?}", v);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                },
                _ => {
                    let msg = format!("acl unknown key: {} = {:?}", k, v);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }
            }
        }

        if access.is_none() {
            let msg = format!("acl access node missing: id={}", id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let ret = AclItem {
            id,

            action,
            res,
            group,
            access: access.unwrap(),
        };

        // 整体检查desc是否有效，是否存在冲突等
        ret.check_desc()?;

        Ok(ret)
    }

    fn check_desc(&self) -> BuckyResult<()> {
        self.group.check_desc(&self.action)?;

        Ok(())
    }
}

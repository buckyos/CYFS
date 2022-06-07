use super::relation::*;
use super::request::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;

use std::str::FromStr;
use toml::Value as Toml;

pub(crate) enum AclGroupProtocol {
    Any,

    // from NONProtocol
    Native,
    Meta,
    Sync,
    HttpBdt,
    HttpLocal,
    DatagramBdt,
    DataBdt,

    Remote,
    Local,
}

impl Default for AclGroupProtocol {
    fn default() -> Self {
        Self::Any
    }
}

impl FromStr for AclGroupProtocol {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "*" => Self::Any,

            "native" => Self::Native,
            "meta" => Self::Meta,
            "sync" => Self::Sync,
            "http-bdt" => Self::HttpBdt,
            "http-local" => Self::HttpLocal,
            "datagram-bdt" => Self::DatagramBdt,
            "data-bdt" => Self::DataBdt,

            "remote" => Self::Remote,
            "local" => Self::Local,

            _ => {
                let msg = format!("unknown acl group protocol: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}

impl AclGroupProtocol {
    pub fn is_match(&self, req: &dyn AclRequest) -> bool {
        match self {
            Self::Any => true,
            Self::Local => req.protocol().is_local(),
            Self::Remote => req.protocol().is_remote(),

            Self::HttpLocal => *req.protocol() == NONProtocol::HttpLocal,
            Self::HttpBdt => *req.protocol() == NONProtocol::HttpBdt,
            Self::DataBdt => *req.protocol() == NONProtocol::DataBdt,
            Self::Native => *req.protocol() == NONProtocol::Native,
            Self::Meta => *req.protocol() == NONProtocol::Meta,
            Self::Sync => *req.protocol() == NONProtocol::Sync,
            Self::DatagramBdt => *req.protocol() == NONProtocol::DatagramBdt,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub(crate) enum AclGroupLocation {
    Any = 0,
    InnerZone = 1,
    OuterZone = 2,
}

impl Default for AclGroupLocation {
    fn default() -> Self {
        Self::Any
    }
}

impl FromStr for AclGroupLocation {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = match s {
            "*" => Self::Any,
            "inner" => Self::InnerZone,
            "outer" => Self::OuterZone,

            _ => {
                let msg = format!("unknown acl group location: {}", s);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };

        Ok(ret)
    }
}

impl AclGroupLocation {
    pub async fn is_match(&self, req: &dyn AclRequest) -> bool {
        match self {
            Self::Any => true,
            _ => {
                match req.location().await {
                    Ok(l) => {
                        assert_ne!(*l, AclGroupLocation::Any);
                        *self == *l
                    }
                    Err(_) => false, // 出错后一律认为不匹配
                }
            }
        }
    }
}

pub struct AclGroupDefaultDecItem {
    id: String,
    // dec_id: DecAppId,
}

impl AclGroupDefaultDecItem {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn is_match(&self, _dec_id: &str) -> bool {
        // TODO 从default_app_manager查询并判断，可以增加缓存
        todo!();
    }
}

pub enum AclGroupDec {
    // *
    Any,

    // 系统应用
    System,

    // 默认应用类别，如im,mail
    DefaultDec(AclGroupDefaultDecItem),

    // 明确的dec
    Dec(DecAppId),
}

impl Default for AclGroupDec {
    fn default() -> Self {
        Self::Any
    }
}

impl AclGroupDec {
    pub fn is_default_dec(_s: &str) -> bool {
        // TODO 默认应用列表
        false
    }
}

impl FromStr for AclGroupDec {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ret = if s == "*" {
            Self::Any
        } else if s == "system" {
            Self::System
        } else if Self::is_default_dec(s) {
            Self::DefaultDec(AclGroupDefaultDecItem::new(s))
        } else {
            // dec_id
            let id = DecAppId::from_str(s).map_err(|e| {
                let msg = format!("invalid acl group dec_id: {}, {}", s, e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

            Self::Dec(id)
        };

        Ok(ret)
    }
}

impl AclGroupDec {
    pub fn is_match(&self, dec_id: &str) -> bool {
        match self {
            Self::Any => true,
            Self::System => {
                // TODO 如何判断一个dec-id是不是系统id？
                todo!();
            }
            Self::DefaultDec(item) => item.is_match(dec_id),
            Self::Dec(id) => id.to_string() == dec_id,
        }
    }
}

pub(crate) enum AclGroupRelation {
    Any,
    Specified(AclDelayRelation),
}

impl Default for AclGroupRelation {
    fn default() -> Self {
        Self::Any
    }
}

impl AclGroupRelation {
    pub async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        match self {
            Self::Any => Ok(true),
            Self::Specified(r) => r.is_match(req).await,
        }
    }
}
pub(crate) struct AclGroup {
    location: AclGroupLocation,
    protocol: AclGroupProtocol,
    dec: AclGroupDec,
    relation: AclGroupRelation,
}

impl Default for AclGroup {
    fn default() -> Self {
        Self {
            location: AclGroupLocation::Any,
            protocol: AclGroupProtocol::Any,
            dec: AclGroupDec::Any,
            relation: AclGroupRelation::Any,
        }
    }
}

impl AclGroup {
    pub async fn is_match(&self, req: &dyn AclRequest) -> BuckyResult<bool> {
        if !self.location.is_match(req).await {
            trace!("location not match!");
            return Ok(false);
        }

        if !self.protocol.is_match(req) {
            trace!("protocol not match!");
            return Ok(false);
        }

        if !self.dec.is_match(req.dec()) {
            trace!("dec not match!");
            return Ok(false);
        }

        self.relation.is_match(req).await
    }

    // acl条目加载完毕后，检查规则配置是否有效
    pub fn check_desc(&self, action: &AclAction) -> BuckyResult<()> {
        match &self.relation {
            AclGroupRelation::Any => Ok(()),
            AclGroupRelation::Specified(relation) => relation.desc().check_valid(action),
        }
    }

    // 支持单字符串和table模式
    // 单字符串： * 表示任意；其余表示relation
    // table: {location = "inner", dec = "", relation = "xxx"}
    pub fn load(relation_manager: &AclRelationManager, value: Toml) -> BuckyResult<Self> {
        match value {
            Toml::Table(t) => Self::load_table(relation_manager, t),
            Toml::String(s) => Self::load_str(relation_manager, &s),
            _ => {
                let msg = format!("acl group node invalid type: {:?}", value);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }

    fn load_table(
        relation_manager: &AclRelationManager,
        table: toml::value::Table,
    ) -> BuckyResult<Self> {
        let mut location = AclGroupLocation::Any;
        let mut protocol = AclGroupProtocol::Any;
        let mut dec = AclGroupDec::Any;
        let mut relation = AclGroupRelation::Any;

        for (k, v) in table.into_iter() {
            match k.as_str() {
                "location" => match v.as_str() {
                    Some(s) => {
                        location = AclGroupLocation::from_str(s)?;
                    }
                    None => {
                        let msg = format!("acl group's location node invalid type: {:?}", v);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                },
                "protocol" => match v.as_str() {
                    Some(s) => {
                        protocol = AclGroupProtocol::from_str(s)?;
                    }
                    None => {
                        let msg = format!("acl group's protocol node invalid type: {:?}", v);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                },
                "dec" => match v.as_str() {
                    Some(s) => {
                        dec = AclGroupDec::from_str(s)?;
                    }
                    None => {
                        let msg = format!("acl group's dec node invalid type: {:?}", v);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                },
                "relation" => match v.as_str() {
                    Some(s) => {
                        let s = s.trim();
                        if s == "*" {
                            relation = AclGroupRelation::Any;
                        } else {
                            relation = AclGroupRelation::Specified(relation_manager.load(s)?);
                        }
                    }
                    None => {
                        let msg = format!("acl group's relation node invalid type: {:?}", v);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                },
                _ => {
                    let msg = format!("acl group's unknown key value: {} = {:?}", k, v);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }
            }
        }

        let ret = Self {
            location,
            protocol,
            dec,
            relation,
        };

        Ok(ret)
    }

    fn load_str(relation_manager: &AclRelationManager, s: &str) -> BuckyResult<Self> {
        let ret = if s == "*" {
            AclGroup::default()
        } else {
            // 解析relation
            AclGroup {
                location: AclGroupLocation::Any,
                protocol: AclGroupProtocol::Any,
                dec: AclGroupDec::Any,
                relation: AclGroupRelation::Specified(relation_manager.load(s)?),
            }
        };

        Ok(ret)
    }
}

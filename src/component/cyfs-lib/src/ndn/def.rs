use cyfs_base::*;

use std::str::FromStr;

use crate::NONAPILevel;



#[derive(Debug, Hash, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum NDNAction {
    PutData,
    GetData,

    DeleteData,
    PutSharedData,
    GetSharedData,

    QueryFile,
}

impl ToString for NDNAction {
    fn to_string(&self) -> String {
        (match *self {

            Self::PutData => "put-data",
            Self::GetData => "get-data",
            Self::DeleteData => "delete-data",
            Self::PutSharedData => "put-shared-data",
            Self::GetSharedData => "get-shared-data",
            Self::QueryFile => "query-file",
        })
        .to_owned()
    }
}

impl FromStr for NDNAction {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "put-data" => Self::PutData,
            "get-data" => Self::GetData,
            "delete-data" => Self::DeleteData,
            "put-shared-data" => Self::PutSharedData,
            "get-shared-data" => Self::GetSharedData,
            "query-file" => Self::QueryFile,
            v @ _ => {
                let msg = format!("unknown ndn action: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

// non操作的缺省行为，默认为NON
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NDNAPILevel {
    NDC = 0,
    NDN = 1,
    Router = 2,
}

impl Into<NONAPILevel> for NDNAPILevel {
    fn into(self) -> NONAPILevel {
        match self {
            Self::NDC => NONAPILevel::NOC,
            Self::NDN => NONAPILevel::NON,
            Self::Router => NONAPILevel::Router,
        }
    }
}

impl Default for NDNAPILevel {
    fn default() -> Self {
        Self::Router
    }
}

impl ToString for NDNAPILevel {
    fn to_string(&self) -> String {
        (match *self {
            Self::NDC => "ndc",
            Self::NDN => "ndn",
            Self::Router => "router",
        })
        .to_owned()
    }
}

impl FromStr for NDNAPILevel {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "ndc" => Self::NDC,
            "ndn" => Self::NDN,
            "router" => Self::Router,
            v @ _ => {
                let msg = format!("unknown ndn api level: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        Ok(ret)
    }
}


#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NDNPutDataResult {
    Accept,
    AlreadyExists,
}

impl ToString for NDNPutDataResult {
    fn to_string(&self) -> String {
        (match *self {
            Self::Accept => "Accept",
            Self::AlreadyExists => "AlreadyExists",
        })
        .to_owned()
    }
}

impl FromStr for NDNPutDataResult {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "Accept" => Self::Accept,
            "AlreadyExists" => Self::AlreadyExists,
            v @ _ => {
                let msg = format!("unknown NDNPutDataResult: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

// 对于chunk，那么需要配置所引用的file_id/dir_id,以及最近的一个dir_id
// dir_id(dir内部直接使用了chunk)
// dir_id/file_id
// file_id(file内部的chunk)

// chunk的关联对象，一般是file/dir+inner_path
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NDNDataRefererObject {
    pub object_id: ObjectId,
    pub inner_path: Option<String>,
}

impl ToString for NDNDataRefererObject {
    fn to_string(&self) -> String {
        if let Some(inner_path) = &self.inner_path {
            format!("{}/{}", self.object_id.to_string(), inner_path)
        } else {
            self.object_id.to_string()
        }
    }
}

impl FromStr for NDNDataRefererObject {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = value.split("/").collect();
        if parts.is_empty() {
            let msg = format!(
                "invalid NDNDataRefererObject, object_id not found! {}",
                value
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        // 第一段一定是object
        let object_id = ObjectId::from_str(parts[0]).map_err(|e| {
            let msg = format!("invalid NDNDataRefererObject object_id format! {}, {}", value, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let inner_path = if parts.len() > 1 {
            Some(parts[..1].join("/"))
        } else {
            None
        };

        Ok(Self {
            object_id,
            inner_path,
        })
    }
}

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

// target 可以是 chunk的owner，或者转发者， 或者是关联的 dsg contract

// chunk的关联对象，一般是[target:]file  或者 [target:]dir/inner_path
#[derive(Clone, Eq, PartialEq)]
pub struct NDNDataRefererObject {
    pub target: Option<ObjectId>,
    pub object_id: ObjectId,
    pub inner_path: Option<String>,
}

impl NDNDataRefererObject {
    pub fn is_inner_path_empty(&self) -> bool {
        match &self.inner_path {
            Some(v) => v.trim().is_empty(),
            None => true,
        }
    }

    pub fn to_string(&self) -> String {
        let last = if let Some(inner_path) = &self.inner_path {
            format!("{}/{}", self.object_id.to_string(), inner_path.trim_start_matches('/'))
        } else {
            self.object_id.to_string()
        };
        if let Some(target) = &self.target {
            format!("{}:{}", target.to_string(), last)
        } else {
            last
        }
    }
}

impl std::fmt::Display for NDNDataRefererObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
    }
}

impl std::fmt::Debug for NDNDataRefererObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_string())
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

        let id_parts: Vec<&str> = parts[0].split(":").collect();
        let (target, object_id) = if id_parts.len() == 1 {
            ObjectId::from_str(id_parts[0])
                .map_err(|e| {
                    let msg = format!(
                        "invalid NDNDataRefererObject object_id format! {}, {}",
                        value, e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })
                .map(|o| (None, o))
        } else if id_parts.len() == 2 {
            ObjectId::from_str(id_parts[0])
                .map_err(|e| {
                    let msg = format!(
                        "invalid NDNDataRefererObject target format! {}, {}",
                        value, e
                    );
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })
                .and_then(|t| {
                    ObjectId::from_str(id_parts[1])
                        .map_err(|e| {
                            let msg = format!(
                                "invalid NDNDataRefererObject object_id format! {}, {}",
                                value, e
                            );
                            error!("{}", msg);
                            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                        })
                        .map(|o| (Some(t), o))
                })
        } else {
            let msg = format!(
                "invalid NDNDataRefererObject, object_id not found! {}",
                value
            );
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
        }?;

        let inner_path = if parts.len() > 1 {
            let inner_path = parts[1..].join("/");
            let inner_path = if inner_path != "/" {
                format!("/{}", inner_path)
            } else {
                inner_path
            };
            Some(inner_path)
        } else {
            None
        };

        Ok(Self {
            target,
            object_id,
            inner_path,
        })
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use cyfs_base::*;
    use std::str::FromStr;

    fn test_codec_impl(inner_path: &str) {
        let target = DeviceId::from_str("5aSixgPJLRApy31v15U8mM7cVSndc8kjbECXSfP9o6Ef").unwrap();
        let object_id = ObjectId::from_str("7jMmeXZcUvyomvtKAnfa8BCdCXCzdp1S5xavMsC7BcYJ").unwrap();

        let referer_object = NDNDataRefererObject {
            target: None,
            object_id: object_id.clone(),
            inner_path: Some(inner_path.to_owned()),
        };

        let s = referer_object.to_string();
        println!("{}", s);

        let referer_object2 = NDNDataRefererObject::from_str(&s).unwrap();
        assert_eq!(referer_object, referer_object2);

        let referer_object = NDNDataRefererObject {
            target: Some(target.object_id().to_owned()),
            object_id: object_id.clone(),
            inner_path: Some(inner_path.to_owned()),
        };

        let s = referer_object.to_string();
        println!("{}", s);

        let referer_object2 = NDNDataRefererObject::from_str(&s).unwrap();
        assert_eq!(referer_object, referer_object2);

        let referer_object = NDNDataRefererObject {
            target: None,
            object_id: object_id.clone(),
            inner_path: None,
        };

        let s = referer_object.to_string();
        println!("{}", s);

        let referer_object2 = NDNDataRefererObject::from_str(&s).unwrap();
        assert_eq!(referer_object, referer_object2);
    }

    #[test]
    fn test_codec() {
        let inner_path = "/a/b/c/ddd";
        test_codec_impl(inner_path);

        let inner_path = "/";
        test_codec_impl(inner_path);

        let inner_path = "/abcd";
        test_codec_impl(inner_path);
    }
}
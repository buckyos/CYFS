use crate::ndn::*;
use cyfs_base::*;

use serde_json::{Map, Value};
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;

// 请求的数据类型
#[derive(Clone)]
pub enum NONDataType {
    // 请求一个object
    Object = 0,

    // 请求对应的数据
    Data = 1,
}

impl ToString for NONDataType {
    fn to_string(&self) -> String {
        (match *self {
            Self::Object => "object",
            Self::Data => "data",
        })
        .to_owned()
    }
}

impl FromStr for NONDataType {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "object" => Self::Object,
            "data" => Self::Data,
            v @ _ => {
                let msg = format!("unknown non datatype: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum NONAction {
    // non
    PutObject,
    GetObject,
    PostObject,
    SelectObject,
    DeleteObject,
}

impl ToString for NONAction {
    fn to_string(&self) -> String {
        (match *self {
            Self::PutObject => "put-object",
            Self::GetObject => "get-object",
            Self::PostObject => "post-object",
            Self::SelectObject => "select-object",
            Self::DeleteObject => "delete-object",
        })
        .to_owned()
    }
}

impl FromStr for NONAction {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "put-object" => Self::PutObject,
            "get-object" => Self::GetObject,
            "post-object" => Self::PostObject,
            "select-object" => Self::SelectObject,
            "delete-object" => Self::DeleteObject,
            v @ _ => {
                let msg = format!("unknown non action: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

// non操作的缺省行为，默认为NON
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NONAPILevel {
    NOC = 0,
    NON = 1,
    Router = 2,
}

impl Default for NONAPILevel {
    fn default() -> Self {
        Self::Router
    }
}

impl Into<NDNAPILevel> for NONAPILevel {
    fn into(self) -> NDNAPILevel {
        match self {
            Self::NOC => NDNAPILevel::NDC,
            Self::NON => NDNAPILevel::NDN,
            Self::Router => NDNAPILevel::Router,
        }
    }
}

impl ToString for NONAPILevel {
    fn to_string(&self) -> String {
        (match *self {
            Self::NON => "non",
            Self::NOC => "noc",
            Self::Router => "router",
        })
        .to_owned()
    }
}

impl FromStr for NONAPILevel {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "non" => Self::NON,
            "noc" => Self::NOC,
            "router" => Self::Router,
            v @ _ => {
                let msg = format!("unknown non api level: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NONPutObjectResult {
    Accept,
    AcceptWithSign,
    AlreadyExists,
    Updated,
    Merged,
}

impl ToString for NONPutObjectResult {
    fn to_string(&self) -> String {
        (match *self {
            Self::Accept => "Accept",
            Self::AcceptWithSign => "AcceptWithSign",
            Self::AlreadyExists => "AlreadyExists",
            Self::Updated => "Updated",
            Self::Merged => "Merged",
        })
        .to_owned()
    }
}

impl FromStr for NONPutObjectResult {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "Accept" => Self::Accept,
            "AcceptWithSign" => Self::AcceptWithSign,
            "AlreadyExists" => Self::AlreadyExists,
            "Updated" => Self::Updated,
            "Merged" => Self::Merged,
            v @ _ => {
                let msg = format!("unknown NONPutObjectResult: {}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };

        Ok(ret)
    }
}

#[derive(Clone, Debug, Copy)]
pub enum NONObjectFormat {
    Default,
    Raw,
    Json,
}

impl FromStr for NONObjectFormat {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "default" => Self::Default,
            "raw" => Self::Raw,
            "json" => Self::Json,
            v @ _ => {
                let msg = format!("unknown NONObjectFormat: {}", v);
                error!("{}", msg);

                Self::Raw
            }
        };

        Ok(ret)
    }
}

#[derive(Clone)]
pub struct NONObjectInfo {
    pub object_id: ObjectId,
    pub object_raw: Vec<u8>,

    // 可选，用以内部直接使用
    pub object: Option<Arc<AnyNamedObject>>,
}

impl NONObjectInfo {
    pub fn new(
        object_id: ObjectId,
        object_raw: Vec<u8>,
        object: Option<Arc<AnyNamedObject>>,
    ) -> Self {
        Self {
            object_id,
            object_raw,
            object,
        }
    }

    pub fn new_from_object_raw(object_raw: Vec<u8>) -> BuckyResult<Self> {
        let (object, _) = AnyNamedObject::raw_decode(&object_raw).map_err(|e| {
            error!("decode object from object_raw error: {}", e,);
            e
        })?;

        let object_id = object.object_id();
        Ok(Self::new(object_id, object_raw, Some(Arc::new(object))))
    }

    pub fn object(&self) -> &Arc<AnyNamedObject> {
        self.object.as_ref().unwrap()
    }

    pub fn take_object(&mut self) -> Arc<AnyNamedObject> {
        self.object.take().unwrap()
    }

    pub fn clone_object(&self) -> Arc<AnyNamedObject> {
        self.object.as_ref().unwrap().clone()
    }
    pub fn try_decode(&mut self) -> BuckyResult<()> {
        if self.object.is_none() {
            self.decode()
        } else {
            Ok(())
        }
    }

    pub fn decode(&mut self) -> BuckyResult<()> {
        assert!(self.object.is_none());

        let (object, _) = AnyNamedObject::raw_decode(&self.object_raw).map_err(|e| {
            error!(
                "decode object from object_raw error: obj={} {}",
                self.object_id, e,
            );
            e
        })?;

        self.object = Some(Arc::new(object));
        Ok(())
    }

    pub fn verify(&self) -> BuckyResult<()> {
        let calc_id = if let Some(object) = &self.object {
            object.calculate_id()
        } else {
            let (object, _) = AnyNamedObject::raw_decode(&self.object_raw).map_err(|e| {
                error!(
                    "decode object from object_raw error: obj={} {}",
                    self.object_id, e,
                );
                e
            })?;

            object.calculate_id()
        };
        
        // 校验id
        if calc_id != self.object_id {
            let msg = format!("unmatch object id: {}, calc={}", self.object_id, calc_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        Ok(())
    }

    pub fn decode_and_verify(&mut self) -> BuckyResult<()> {
        self.decode()?;
        self.verify()
    }

    pub fn get_update_time(&mut self) -> BuckyResult<u64> {
        self.try_decode()?;

        let object = self.object.as_ref().unwrap();
        let t = object.get_update_time();
        if t > 0 {
            debug!("object update time: {}, {}", self.object_id, t);
        }

        Ok(t)
    }

    pub fn get_expired_time(&mut self) -> BuckyResult<Option<u64>> {
        self.try_decode()?;

        let object = self.object.as_ref().unwrap();
        let t = object.expired_time();
        if let Some(t) = &t {
            debug!("object expired time: {}, {}", self.object_id, t);
        }

        Ok(t)
    }
}

impl fmt::Debug for NONObjectInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for NONObjectInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "object_id: {}, len: {}",
            self.object_id,
            self.object_raw.len(),
        )?;

        if let Some(obj) = &self.object {
            write!(
                f,
                ", obj_type: {}",
                obj.obj_type()
            )?;
        }

        Ok(())
    }
}

impl JsonCodec<NONObjectInfo> for NONObjectInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert(
            "object_raw".to_owned(),
            Value::String(hex::encode(&self.object_raw)),
        );

        obj.insert(
            "object_id".to_owned(),
            Value::String(self.object_id.to_string()),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONObjectInfo> {
        let object_id: ObjectId = JsonCodecHelper::decode_string_field(obj, "object_id")?;

        let object_raw: String = JsonCodecHelper::decode_string_field(obj, "object_raw")?;
        let object_raw = hex::decode(&object_raw).map_err(|e| {
            let msg = format!("invalid object_raw hex buffer! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let mut object = NONObjectInfo::new(object_id, object_raw, None);
        object.decode_and_verify()?;

        Ok(object)
    }
}


#[derive(Clone)]
pub struct NONSlimObjectInfo {
    pub object_id: ObjectId,
    pub object_raw: Option<Vec<u8>>,
    pub object: Option<Arc<AnyNamedObject>>,
}

impl fmt::Debug for NONSlimObjectInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "object_id: {}", self.object_id)?;
        if let Some(object_raw) = &self.object_raw {
            write!(f, ", len: {}", object_raw.len())?;
        }

        Ok(())
    }
}

impl NONSlimObjectInfo {
    pub fn new(object_id: ObjectId, object_raw: Option<Vec<u8>>, object: Option<Arc<AnyNamedObject>>) -> Self {
        Self {
            object_id,
            object_raw,
            object,
        }
    }

    pub fn decode(&mut self) -> BuckyResult<()> {
        assert!(self.object.is_none());

        if let Some(object_raw) = &self.object_raw {
            let (object, _) = AnyNamedObject::raw_decode(&object_raw).map_err(|e| {
                error!(
                    "decode object from object_raw error: obj={} {}",
                    self.object_id, e,
                );
                e
            })?;
            self.object = Some(Arc::new(object));
        }
        Ok(())
    }

    pub fn verify(&self) -> BuckyResult<()> {
        let calc_id = if let Some(object) = &self.object {
            object.calculate_id()
        } else if let Some(object_raw) = &self.object_raw {
            let (object, _) = AnyNamedObject::raw_decode(&object_raw).map_err(|e| {
                error!(
                    "decode object from object_raw error: obj={} {}",
                    self.object_id, e,
                );
                e
            })?;

            object.calculate_id()
        } else {
            return Ok(());
        };

        // 校验id
        if calc_id != self.object_id {
            let msg = format!("unmatch object id: {}, calc={}", self.object_id, calc_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        Ok(())
    }

    pub fn decode_and_verify(&mut self) -> BuckyResult<()> {
        if self.object_raw.is_some() && self.object.is_none() {
            self.decode()?;
        }
        
        self.verify()
    }
}

impl JsonCodec<NONSlimObjectInfo> for NONSlimObjectInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        if let Some(object_raw) = &self.object_raw {
            obj.insert(
                "object_raw".to_owned(),
                Value::String(hex::encode(object_raw)),
            );
        } else if let Some(object) = &self.object {
            let object_raw = object.to_vec().unwrap();
            obj.insert(
                "object_raw".to_owned(),
                Value::String(hex::encode(object_raw)),
            );
        }

        obj.insert(
            "object_id".to_owned(),
            Value::String(self.object_id.to_string()),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let object_id: ObjectId = JsonCodecHelper::decode_string_field(obj, "object_id")?;

        let object_raw: Option<String> =
            JsonCodecHelper::decode_option_string_field(obj, "object_raw")?;
        let object_raw = if let Some(object_raw) = object_raw {
            let object_raw = hex::decode(&object_raw).map_err(|e| {
                let msg = format!("invalid object_raw hex buffer! {}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

            Some(object_raw)
        } else {
            None
        };

        let mut object = Self::new(object_id, object_raw, None);
        object.decode_and_verify()?;

        Ok(object)
    }
}
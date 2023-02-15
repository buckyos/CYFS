use crate::*;

use std::str::FromStr;

#[derive(Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum ObjectTypeCode {
    Device = 1,
    People = 2,
    Group = 3,
    AppGroup = 5,
    UnionAccount = 6,
    Chunk = 7,
    File = 8,
    Dir = 9,
    Diff = 10,
    ProofOfService = 11,
    Tx = 12,
    Action = 13,
    ObjectMap = 14,
    Contract = 15,
    Custom = 16,
}

impl From<u16> for ObjectTypeCode {
    fn from(v: u16) -> Self {
        match v {
            1u16 => ObjectTypeCode::Device,
            2u16 => ObjectTypeCode::People,
            3u16 => ObjectTypeCode::Group,
            5u16 => ObjectTypeCode::AppGroup,
            6u16 => ObjectTypeCode::UnionAccount,
            7u16 => ObjectTypeCode::Chunk,
            8u16 => ObjectTypeCode::File,
            9u16 => ObjectTypeCode::Dir,
            10u16 => ObjectTypeCode::Diff,
            11u16 => ObjectTypeCode::ProofOfService,
            12u16 => ObjectTypeCode::Tx,
            13u16 => ObjectTypeCode::Action,
            14u16 => ObjectTypeCode::ObjectMap,
            15u16 => ObjectTypeCode::Contract,
            16u16 => ObjectTypeCode::Custom,
            _ => ObjectTypeCode::Custom,
        }
    }
}

impl From<&ObjectTypeCode> for u16 {
    fn from(v: &ObjectTypeCode) -> Self {
        match v {
            ObjectTypeCode::Device => 1u16,
            ObjectTypeCode::People => 2u16,
            ObjectTypeCode::Group => 3u16,
            ObjectTypeCode::AppGroup => 5u16,
            ObjectTypeCode::UnionAccount => 6u16,
            ObjectTypeCode::Chunk => 7u16,
            ObjectTypeCode::File => 8u16,
            ObjectTypeCode::Dir => 9u16,
            ObjectTypeCode::Diff => 10u16,
            ObjectTypeCode::ProofOfService => 11u16,
            ObjectTypeCode::Tx => 12u16,
            ObjectTypeCode::Action => 13u16,
            ObjectTypeCode::ObjectMap => 14u16,
            ObjectTypeCode::Contract => 15u16,
            _ => 16u16,
        }
    }
}

impl From<ObjectTypeCode> for u16 {
    fn from(v: ObjectTypeCode) -> Self {
        let r = &v;
        r.into()
    }
}

impl ObjectTypeCode {
    pub fn raw_check_type_code(buf: &[u8]) -> Self {
        let flag = buf[0];
        if (flag >> 6) != 0b_00000001 {
            ObjectTypeCode::Custom
        } else {
            let obj_type_code = (flag << 2 >> 4) as u16;
            obj_type_code.into()
        }
    }

    pub fn to_u16(&self) -> u16 {
        self.into()
    }

    pub fn to_u8(&self) -> u8 {
        self.to_u16() as u8
    }
}

impl FromStr for ObjectTypeCode {
    type Err = BuckyError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let v: u16 = s.parse().map_err(|e| {
            warn!("parse object type code error: {} {}", s, e);
            BuckyError::from(BuckyErrorCode::InvalidFormat)
        })?;

        Ok(v.into())
    }
}

impl ToString for ObjectTypeCode {
    fn to_string(&self) -> String {
        let v: u16 = self.into();
        v.to_string()
    }
}

impl RawEncode for ObjectTypeCode {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(u16::raw_bytes().unwrap())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let v: u16 = self.into();
        v.raw_encode(buf, purpose)
    }
}

impl<'de> RawDecode<'de> for ObjectTypeCode {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (v, buf) = u16::raw_decode(buf).map_err(|e| {
            log::error!("ObjectTypeCode::raw_decode error:{}", e);
            e
        })?;
        let v = v.into();

        Ok((v, buf))
    }
}

pub mod object_type_helper {
    use super::*;

    pub fn is_standard_object(object_type: u16) -> bool {
        object_type >= OBJECT_TYPE_STANDARD_START && object_type <= OBJECT_TYPE_STANDARD_END
    }

    pub fn is_custom_object(object_type: u16) -> bool {
        object_type >= OBJECT_TYPE_CORE_START
    }

    pub fn is_core_object(object_type: u16) -> bool {
        object_type >= OBJECT_TYPE_CORE_START && object_type <= OBJECT_TYPE_CORE_END
    }

    pub fn is_dec_app_object(object_type: u16) -> bool {
        object_type >= OBJECT_TYPE_DECAPP_START && object_type <= OBJECT_TYPE_DECAPP_END
    }
}

// object 类型trait的基础，为每种object都要实现这个
pub trait ObjectType: Clone {
    // 如果是非标准对象：ObjectTypeCode::Custome
    fn obj_type_code() -> ObjectTypeCode;

    // 标准对象：obj_type < 17
    // 核心对象：obj_type >= 17 && obj_type < 2^15
    // DecApp对象：obj_type >= 2^15 && obj_type < 2^16
    fn obj_type() -> u16;

    fn is_standard_object() -> bool {
        let c = Self::obj_type_code();
        c != ObjectTypeCode::Custom
    }

    fn is_core_object() -> bool {
        let t = Self::obj_type();
        let c = Self::obj_type_code();
        c == ObjectTypeCode::Custom && object_type_helper::is_core_object(t)
    }

    fn is_dec_app_object() -> bool {
        let t = Self::obj_type();
        let c = Self::obj_type_code();
        c == ObjectTypeCode::Custom && object_type_helper::is_dec_app_object(t)
    }

    type DescType: ObjectDesc + Sync + Send;
    type ContentType: Sync + Send;
}

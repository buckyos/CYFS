use crate::*;

use base58::{FromBase58, ToBase58};
use generic_array::typenum::{marker_traits::Unsigned, U32};
use generic_array::GenericArray;
use protobuf::Clear;
use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ObjectCategory {
    Standard,
    Core,
    DecApp,
}

impl ToString for ObjectCategory {
    fn to_string(&self) -> String {
        (match *self {
            ObjectCategory::Standard => "standard",
            ObjectCategory::Core => "core",
            ObjectCategory::DecApp => "dec_app",
        })
        .to_owned()
    }
}

impl FromStr for ObjectCategory {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "standard" => ObjectCategory::Standard,
            "core" => ObjectCategory::Core,
            "dec_app" => ObjectCategory::DecApp,
            v @ _ => {
                let msg = format!("unknown object category: {}", v);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        Ok(ret)
    }
}

use std::ops::Range;

// roundup(32 * log(256) / log(58)) 43.7
pub const OBJECT_ID_BASE58_RANGE: Range<usize> = 43..45;

// roundup(32 * log(256) / log(36)) 49.5
pub const OBJECT_ID_BASE36_RANGE: Range<usize> = 49..51;

// 包含objec type的 object id
#[derive(Copy, Clone, PartialOrd, PartialEq, Ord, Eq)]
pub struct ObjectId(GenericArray<u8, U32>);

impl std::fmt::Debug for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl From<[u8; 32]> for ObjectId {
    fn from(v: [u8; 32]) -> Self {
        Self(GenericArray::from(v))
    }
}

impl From<Vec<u8>> for ObjectId {
    fn from(v: Vec<u8>) -> Self {
        let ar: [u8; 32] = v.try_into().unwrap_or_else(|v: Vec<u8>| {
            panic!(
                "ObjectId expected a Vec of length {} but it was {}",
                32,
                v.len()
            )
        });

        Self(GenericArray::from(ar))
    }
}

impl From<GenericArray<u8, U32>> for ObjectId {
    fn from(hash: GenericArray<u8, U32>) -> Self {
        Self(hash)
    }
}

impl From<ObjectId> for GenericArray<u8, U32> {
    fn from(hash: ObjectId) -> Self {
        hash.0
    }
}

impl From<H256> for ObjectId {
    fn from(val: H256) -> Self {
        ObjectId::clone_from_slice(val.as_ref())
    }
}

impl Into<H256> for ObjectId {
    fn into(self) -> H256 {
        H256::from_slice(self.as_slice())
    }
}

impl AsRef<GenericArray<u8, U32>> for ObjectId {
    fn as_ref(&self) -> &GenericArray<u8, U32> {
        &self.0
    }
}

impl Default for ObjectId {
    fn default() -> Self {
        ObjectId(GenericArray::default())
    }
}

impl ProtobufTransform<ObjectId> for Vec<u8> {
    fn transform(value: ObjectId) -> BuckyResult<Self> {
        Ok(Vec::from(value.0.as_slice()))
    }
}

impl ProtobufTransform<&ObjectId> for Vec<u8> {
    fn transform(value: &ObjectId) -> BuckyResult<Self> {
        Ok(Vec::from(value.0.as_slice()))
    }
}

impl ProtobufTransform<Vec<u8>> for ObjectId {
    fn transform(value: Vec<u8>) -> BuckyResult<Self> {
        if value.len() != 32 {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidParam,
                format!(
                    "try convert from vec<u8> to named object id failed, invalid len {}",
                    value.len()
                ),
            ));
        }
        let mut id = Self::default();
        unsafe {
            std::ptr::copy(value.as_ptr(), id.as_mut_slice().as_mut_ptr(), value.len());
        }

        Ok(id)
    }
}

pub const OBJECT_ID_STANDARD: u8 = 0b_00000001;
pub const OBJECT_ID_CORE: u8 = 0b_00000010;
pub const OBJECT_ID_DEC_APP: u8 = 0b_00000011;

pub const OBJECT_ID_FLAG_AREA: u8 = 0b_001000;
pub const OBJECT_ID_FLAG_PK: u8 = 0b_000100;
pub const OBJECT_ID_FLAG_MN_PK: u8 = 0b_000010;
pub const OBJECT_ID_FLAG_OWNER: u8 = 0b_000001;

pub struct StandardObjectIdInfo {
    pub obj_type_code: ObjectTypeCode,
    pub obj_type: u16,
    pub area: Option<Area>,
}

pub struct CoreObjectIdInfo {
    pub area: Option<Area>,
    pub has_owner: bool,
    pub has_single_key: bool,
    pub has_mn_key: bool,
}

pub struct DecAppObjectIdInfo {
    pub area: Option<Area>,
    pub has_owner: bool,
    pub has_single_key: bool,
    pub has_mn_key: bool,
}

pub enum ObjectIdInfo {
    Standard(StandardObjectIdInfo),
    Core(CoreObjectIdInfo),
    DecApp(DecAppObjectIdInfo),
}

impl ObjectIdInfo {
    pub fn area(&self) -> &Option<Area> {
        match self {
            Self::Standard(v) => &v.area,
            Self::Core(v) => &v.area,
            Self::DecApp(v) => &v.area,
        }
    }

    pub fn into_area(self) -> Option<Area> {
        match self {
            Self::Standard(v) => v.area,
            Self::Core(v) => v.area,
            Self::DecApp(v) => v.area,
        }
    }
}

pub struct ObjectIdBuilder<'a, T>
where
    T: RawEncode + ObjectDesc,
{
    t: &'a T,
    obj_type_code: ObjectTypeCode,
    area: Option<&'a Area>,
    has_owner: bool,
    has_single_key: bool,
    has_mn_key: bool,
}

impl<'a, T> ObjectIdBuilder<'a, T>
where
    T: RawEncode + ObjectDesc,
{
    pub fn new(t: &'a T, obj_type_code: ObjectTypeCode) -> Self {
        Self {
            t,
            obj_type_code,
            area: None,
            has_owner: false,
            has_single_key: false,
            has_mn_key: false,
        }
    }

    pub fn area(mut self, area: Option<&'a Area>) -> Self {
        self.area = area;
        self
    }

    pub fn owner(mut self, value: bool) -> Self {
        self.has_owner = value;
        self
    }

    pub fn single_key(mut self, value: bool) -> Self {
        self.has_single_key = value;
        self
    }

    pub fn mn_key(mut self, value: bool) -> Self {
        self.has_mn_key = value;
        self
    }

    pub fn build(self) -> ObjectId {
        let mut hash = self.t.raw_hash_value().unwrap();
        let hash_value = hash.as_mut_slice();

        // TODO:u64数组优化
        // 清空前 40 bit
        hash_value[0] = 0;
        hash_value[1] = 0;
        hash_value[2] = 0;
        hash_value[3] = 0;
        hash_value[4] = 0;

        if !self.t.is_stand_object() {
            // 用户类型
            //4个可用flag
            let mut type_code = if self.t.obj_type() > OBJECT_TYPE_CORE_END {
                //这是一个dec app 对象
                //高2bits固定为11
                0b_110000
            } else {
                //这是一个core 对象
                //高2bits固定为10，
                0b_100000
            };

            //| 是否有area_code | 是否有public_key | 是否是多Key对象 | 是否有owner |
            if self.area.is_some() {
                type_code = type_code | 0b_001000;
            }

            if self.has_single_key {
                type_code = type_code | 0b_000100;
            }

            if self.has_mn_key {
                type_code = type_code | 0b_000010;
            }

            if self.has_owner {
                type_code = type_code | 0b_000001;
            }

            if self.area.is_some() {
                let area = self.area.as_ref().unwrap();
                // --------------------------------------------
                // (2bit)(4bit)(国家编码9bits)+(运营商编码4bits)+城市编码(13bits)+inner(8bits) = 40 bit
                // --------------------------------------------
                // 0 obj_bits[. .]type_code[. . . .] country[. .]
                // 1 country[. . . . . .]carrier[x x x x . .]
                // 2 carrier[. .]city[0][x x . . . . . . ]
                // 3 city[1][. . . . . . . . ]
                // 4 inner[. . . . . . . . ]
                hash_value[0] = type_code << 2 | (area.country << 7 >> 14) as u8;
                hash_value[1] = (area.country << 1) as u8 | area.carrier << 4 >> 7;
                hash_value[2] = area.carrier << 5 | (area.city >> 8) as u8;
                hash_value[3] = (area.city << 8 >> 8) as u8;
                hash_value[4] = area.inner;
            } else {
                // 前 6 bit 写入类型信息
                hash_value[0] = type_code << 2;
            }
        } else {
            // 标准类型
            // 6bits的类型(高2bits固定为01，4bits的内置对象类型）+ option<34bits>的区域编码构成
            let type_code = self.obj_type_code.to_u8();

            if self.area.is_some() {
                // --------------------------------------------
                // (2bit)(4bit)(国家编码9bits)+(运营商编码4bits)+城市编码(13bits)+inner(8bits) = 40 bit
                // --------------------------------------------
                // 0 obj_bits[. .]type_code[. . . .] country[. .]
                // 1 country[. . . . . .]carrier[x x x x . .]
                // 2 carrier[. .]city[0][x x . . . . . . ]
                // 3 city[1][. . . . . . . . ]
                // 4 inner[. . . . . . . . ]
                let area = self.area.as_ref().unwrap();
                hash_value[0] = 0b_01000000 | type_code << 4 >> 2 | (area.country << 7 >> 14) as u8;
                hash_value[1] = (area.country << 1) as u8 | area.carrier << 4 >> 7;
                hash_value[2] = area.carrier << 5 | (area.city >> 8) as u8;
                hash_value[3] = (area.city << 8 >> 8) as u8;
                hash_value[4] = area.inner;
            } else {
                hash_value[0] = 0b_01000000 | type_code << 4 >> 2;
            }
        };

        drop(hash_value);
        let id = ObjectId::new(hash.into());

        id
    }
}

#[derive(Copy, Clone, PartialOrd, PartialEq, Ord, Eq)]
pub struct ObjectIdDistance(GenericArray<u8, U32>);

impl Into<u32> for ObjectIdDistance {
    fn into(self) -> u32 {
        let mut last = [0u8; 4];
        last.copy_from_slice(&self.0.as_slice()[28..]);
        u32::from_le_bytes(last)
    }
}

impl Into<u128> for ObjectIdDistance {
    fn into(self) -> u128 {
        let mut last = [0u8; 16];
        last.copy_from_slice(&self.0.as_slice()[16..]);
        u128::from_le_bytes(last)
    }
}

impl ObjectId {
    pub fn obj_type_code(&self) -> ObjectTypeCode {
        ObjectTypeCode::raw_check_type_code(self.as_slice())
    }

    pub fn info(&self) -> ObjectIdInfo {
        let buf = self.as_slice();
        let flag = buf[0];

        let decode_flag = |buf: &[u8]| -> (bool, bool, bool, bool) {
            let type_code = buf[0] << 2 >> 4;
            let has_area = type_code & OBJECT_ID_FLAG_AREA == OBJECT_ID_FLAG_AREA;
            let has_single_key = type_code & OBJECT_ID_FLAG_PK == OBJECT_ID_FLAG_PK;
            let has_mn_key = type_code & OBJECT_ID_FLAG_MN_PK == OBJECT_ID_FLAG_MN_PK;
            let has_owner = type_code & OBJECT_ID_FLAG_OWNER == OBJECT_ID_FLAG_OWNER;
            (has_area, has_single_key, has_mn_key, has_owner)
        };

        let decode_rea = |buf: &[u8]| -> Option<Area> {
            // --------------------------------------------
            // (2bit)(4bit)(国家编码9bits)+(运营商编码4bits)+城市编码(13bits)+inner(8bits) = 34 bit
            // --------------------------------------------
            // 0 obj_bits[. .]type_code[. . . .] country[. .]
            // 1 country[. . . . . .]carrier[x x x x . .]
            // 2 carrier[. .]city[0][x x . . . . . . ]
            // 3 city[1][. . . . . . . . ]
            // 4 inner[. . . . . . . . ]

            let country = (((buf[0] as u16) & 0x3) << 7) | ((buf[1] >> 1) as u16);
            let carrier = (buf[1] << 7 >> 4) | (buf[2] >> 5);
            let city = ((buf[2] as u16) << 11 >> 3) | (buf[3] as u16);
            let inner = buf[4];

            Some(Area::new(country, carrier, city, inner))
        };

        let try_decode_rea = |buf: &[u8]| -> Option<Area> {
            if buf[1] == 0b_00000000
                && buf[2] == 0b_00000000
                && buf[3] == 0b_00000000
                && buf[4] == 0b_00000000
            {
                None
            } else {
                decode_rea(buf)
            }
        };

        let obj_bits = flag >> 6;

        match obj_bits {
            OBJECT_ID_STANDARD => {
                // 标准对象
                let obj_type = (flag << 2 >> 4) as u16;
                let obj_type_code = obj_type.into();
                let area = try_decode_rea(buf);

                ObjectIdInfo::Standard(StandardObjectIdInfo {
                    obj_type_code,
                    obj_type,
                    area,
                })
            }
            OBJECT_ID_CORE => {
                // 核心对象
                let (has_area, has_single_key, has_mn_key, has_owner) = decode_flag(buf);
                let area = if has_area { decode_rea(buf) } else { None };

                ObjectIdInfo::Core(CoreObjectIdInfo {
                    has_single_key,
                    has_mn_key,
                    has_owner,
                    area,
                })
            }
            OBJECT_ID_DEC_APP => {
                // Dec App 对象
                let (has_area, has_single_key, has_mn_key, has_owner) = decode_flag(buf);
                let area = if has_area { decode_rea(buf) } else { None };

                ObjectIdInfo::DecApp(DecAppObjectIdInfo {
                    has_single_key,
                    has_mn_key,
                    has_owner,
                    area,
                })
            }
            _ => {
                unreachable!();
            }
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }

    pub fn new(inner: GenericArray<u8, U32>) -> Self {
        Self(inner)
    }

    pub fn clone_from_slice(slice: &[u8]) -> Self {
        ObjectId(GenericArray::clone_from_slice(slice))
    }

    pub fn to_string(&self) -> String {
        self.0.as_slice().to_base58()
    }

    pub fn to_hash_value(&self) -> HashValue {
        self.0.as_slice().into()
    }

    pub fn from_base58(s: &str) -> BuckyResult<Self> {
        let buf = s.from_base58().map_err(|e| {
            let msg = format!("convert base58 str to object id failed, str={}, {:?}", s, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        if buf.len() != 32 {
            let msg = format!(
                "convert base58 str to object id failed, len unmatch: str={}",
                s
            );
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(Self::from(buf))
    }

    pub fn to_base36(&self) -> String {
        self.0.as_slice().to_base36()
    }

    pub fn from_base36(s: &str) -> BuckyResult<Self> {
        let buf = s.from_base36()?;
        if buf.len() != 32 {
            let msg = format!(
                "convert base36 str to object id failed, len unmatch: str={}",
                s
            );
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(Self::from(buf))
    }

    pub fn object_category(&self) -> ObjectCategory {
        if self.is_stand_object() {
            ObjectCategory::Standard
        } else if self.is_core_object() {
            ObjectCategory::Core
        } else {
            ObjectCategory::DecApp
        }
    }

    pub fn is_stand_object(&self) -> bool {
        let buf = self.as_slice();
        let flag = buf[0];
        flag >> 6 == 0b_00000001
    }

    pub fn is_core_object(&self) -> bool {
        let buf = self.as_slice();
        let flag = buf[0];
        flag >> 6 == 0b_00000010
    }

    pub fn is_dec_app_object(&self) -> bool {
        let buf = self.as_slice();
        let flag = buf[0];
        flag >> 6 == 0b_00000011
    }

    pub fn distance_of(&self, other: &Self) -> ObjectIdDistance {
        let mut v = GenericArray::<u8, U32>::default();
        for (i, (l, r)) in self
            .0
            .as_slice()
            .iter()
            .zip(other.0.as_slice().iter())
            .enumerate()
        {
            v[i] = *l ^ *r;
        }
        ObjectIdDistance(v)
    }

    pub fn as_chunk_id(&self) -> &ChunkId {
        unsafe { std::mem::transmute::<&ObjectId, &ChunkId>(&self) }
    }
}

impl RawFixedBytes for ObjectId {
    fn raw_bytes() -> Option<usize> {
        Some(U32::to_usize())
    }
}

impl RawEncode for ObjectId {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(U32::to_usize())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for encode ObjectId, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        unsafe {
            std::ptr::copy(self.as_slice().as_ptr(), buf.as_mut_ptr(), bytes);
        }

        Ok(&mut buf[bytes..])
    }
}

impl<'de> RawDecode<'de> for ObjectId {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for decode ObjectId, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        let mut _id = Self::default();
        unsafe {
            std::ptr::copy(buf.as_ptr(), _id.as_mut_slice().as_mut_ptr(), bytes);
        }
        Ok((_id, &buf[bytes..]))
    }
}

impl FromStr for ObjectId {
    type Err = BuckyError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if OBJECT_ID_BASE36_RANGE.contains(&s.len()) {
            Self::from_base36(s)
        } else {
            Self::from_base58(s)
        }
    }
}

impl std::fmt::Display for ObjectId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl Hash for ObjectId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut buff = [0 as u8; 32];
        let _ = self.raw_encode(buff.as_mut(), &None).unwrap();
        state.write(buff.as_ref());
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct ObjectLink {
    pub obj_id: ObjectId,
    pub obj_owner: Option<ObjectId>,
}

impl RawEncode for ObjectLink {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(ObjectId::raw_bytes().unwrap()
            + self.obj_owner.raw_measure(purpose).map_err(|e| {
                log::error!("ObjectId::raw_measure error:{}", e);
                e
            })?)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let bytes = self.raw_measure(purpose).unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for encode ObjectLink, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        let buf = self.obj_id.raw_encode(buf, purpose).map_err(|e| {
            log::error!("ObjectId::raw_encode/obj_id error:{}", e);
            e
        })?;

        let buf = self.obj_owner.raw_encode(buf, purpose).map_err(|e| {
            log::error!("ObjectId::raw_encode/obj_owner error:{}", e);
            e
        })?;

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for ObjectLink {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (obj_id, buf) = ObjectId::raw_decode(buf).map_err(|e| {
            log::error!("ObjectId::raw_decode/obj_id error:{}", e);
            e
        })?;

        let (obj_owner, buf) = Option::<ObjectId>::raw_decode(buf).map_err(|e| {
            log::error!("ObjectId::raw_decode/obj_owner error:{}", e);
            e
        })?;

        Ok((Self { obj_id, obj_owner }, buf))
    }
}

/// 强类型命名对象Id
/// ===
/// 手工实现PartialEq、Eq、PartialOrd、Ord，附加的类型上并不需要实现这四个Trait
/// 转发这四个Trait的实现给内部的ObjectId
#[derive(Copy, Clone)]
pub struct NamedObjectId<T: ObjectType>(ObjectId, Option<PhantomData<T>>);

impl<T: ObjectType> PartialEq for NamedObjectId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T: ObjectType> PartialEq<ObjectId> for NamedObjectId<T> {
    fn eq(&self, other: &ObjectId) -> bool {
        self.0 == *other
    }
}

impl<T: ObjectType> PartialEq<NamedObjectId<T>> for ObjectId {
    fn eq(&self, other: &NamedObjectId<T>) -> bool {
        *self == other.0
    }
}

impl<T: ObjectType> Eq for NamedObjectId<T> {}

impl<T: ObjectType> PartialOrd for NamedObjectId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<T: ObjectType> Ord for NamedObjectId<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T: ObjectType> NamedObjectId<T> {
    pub fn object_id(&self) -> &ObjectId {
        &self.0
    }
    pub fn object_id_mut(&mut self) -> &mut ObjectId {
        &mut self.0
    }
}

impl<T: ObjectType> std::convert::Into<ObjectId> for NamedObjectId<T> {
    fn into(self) -> ObjectId {
        self.0
    }
}

impl<T: ObjectType> Default for NamedObjectId<T> {
    fn default() -> Self {
        let mut id = ObjectId::default();
        let hash_value = id.as_mut_slice();

        if !T::is_stand_object() {
            // 用户类型
            //4个可用flag
            let type_code = if T::is_dec_app_object() {
                //这是一个dec app 对象
                0b_110000
            } else {
                //这是一个core 对象
                0b_100000
            };

            // 前 6 bit 写入类型信息
            hash_value[0] = type_code << 2;
        } else {
            // 标准类型
            let type_code = T::obj_type() as u8;

            hash_value[0] = 0b_01000000 | type_code << 4 >> 2;
        };

        let id = ObjectId::clone_from_slice(&hash_value);

        Self(id, None)
    }
}

impl<T: ObjectType> std::fmt::Display for NamedObjectId<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.to_string())
    }
}

impl<T: ObjectType> std::fmt::Debug for NamedObjectId<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_string())
        /*
        write!(
            f,
            "(obj_id: {:?}, obj_type_code: {:?}, obj_type:{})",
            self.0.as_slice().to_base58(),
            T::obj_type_code(),
            T::obj_type(),
        )
        */
    }
}

impl<T: ObjectType> ProtobufTransform<Vec<u8>> for NamedObjectId<T> {
    fn transform(value: Vec<u8>) -> BuckyResult<Self> {
        Ok(Self(ObjectId::transform(value)?, None))
    }
}

impl<T: ObjectType> ProtobufTransform<&NamedObjectId<T>> for Vec<u8> {
    fn transform(value: &NamedObjectId<T>) -> BuckyResult<Self> {
        Ok(ProtobufTransform::transform(&value.0)?)
    }
}

impl<T: ObjectType> Hash for NamedObjectId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.0.as_slice());
    }
}

impl<T: ObjectType> FromStr for NamedObjectId<T> {
    type Err = BuckyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(ObjectId::from_str(s).map_err(|e| {
            log::error!("NamedObjectId::from_str error:{}", e);
            e
        })?)
    }
}

impl<T: ObjectType> TryFrom<&ObjectId> for NamedObjectId<T> {
    type Error = BuckyError;

    fn try_from(id: &ObjectId) -> Result<Self, Self::Error> {
        let obj_type_code = id.obj_type_code();

        if obj_type_code == T::obj_type_code() {
            Ok(Self(*id, None))
        } else {
            Err(
                BuckyError::new(
                    BuckyErrorCode::InvalidParam,
                    format!("try convert from object id to named object id failed, mismatch obj_type_code, expect obj_type_code is: {:?}, current obj_type_code is: {:?}", T::obj_type_code(), obj_type_code)
                )
            )
        }
    }
}

impl<T: ObjectType> TryFrom<ObjectId> for NamedObjectId<T> {
    type Error = BuckyError;

    fn try_from(id: ObjectId) -> Result<Self, Self::Error> {
        let obj_type_code = id.obj_type_code();

        if obj_type_code == T::obj_type_code() {
            Ok(Self(id, None))
        } else {
            Err(
                BuckyError::new(
                    BuckyErrorCode::InvalidParam,
                    format!("try convert from object id to named object id failed, mismatch obj_type_code, expect obj_type_code is: {}, current obj_type_code is:{}", T::obj_type_code().to_string(), obj_type_code.to_string())
                )
            )
        }
    }
}

impl<T: ObjectType> AsRef<ObjectId> for NamedObjectId<T> {
    fn as_ref(&self) -> &ObjectId {
        &self.0
    }
}

impl<T: ObjectType> RawFixedBytes for NamedObjectId<T> {
    fn raw_bytes() -> Option<usize> {
        ObjectId::raw_bytes()
    }
}

impl<T: ObjectType> RawEncode for NamedObjectId<T> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        self.0.raw_measure(purpose)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        self.0.raw_encode(buf, purpose)
    }
}

impl<'de, T: ObjectType> RawDecode<'de> for NamedObjectId<T> {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (id, buf) = ObjectId::raw_decode(buf)?;
        Ok((
            Self::try_from(id).map_err(|e| {
                log::error!("NamedObjectId::raw_decode/try_from error:{}", e);
                e
            })?,
            buf,
        ))
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum ObjectTypeCode {
    Device = 1,
    People = 2,
    SimpleGroup = 3,
    Org = 4,
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
            3u16 => ObjectTypeCode::SimpleGroup,
            4u16 => ObjectTypeCode::Org,
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
            ObjectTypeCode::SimpleGroup => 3u16,
            ObjectTypeCode::Org => 4u16,
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

    pub fn is_stand_object(object_type: u16) -> bool {
        object_type >= OBJECT_TYPE_STANDARD_START && object_type <= OBJECT_TYPE_STANDARD_END
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

    fn is_stand_object() -> bool {
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

pub trait ObjectDesc {
    fn obj_type(&self) -> u16;

    // 默认实现，从obj_type 转 obj_type_code
    fn obj_type_code(&self) -> ObjectTypeCode {
        let t = self.obj_type();
        t.into()
    }

    fn is_stand_object(&self) -> bool {
        let c = self.obj_type_code();
        c != ObjectTypeCode::Custom
    }

    fn is_core_object(&self) -> bool {
        let t = self.obj_type();
        let c = self.obj_type_code();
        c == ObjectTypeCode::Custom && (t >= OBJECT_TYPE_CORE_START && t <= OBJECT_TYPE_CORE_END)
    }

    fn is_dec_app_object(&self) -> bool {
        let t = self.obj_type();
        let c = self.obj_type_code();
        c == ObjectTypeCode::Custom
            && (t >= OBJECT_TYPE_DECAPP_START && t <= OBJECT_TYPE_DECAPP_END)
    }

    fn object_id(&self) -> ObjectId {
        self.calculate_id()
    }
    // 计算 id
    fn calculate_id(&self) -> ObjectId;

    // 获取所属 DECApp 的 id
    fn dec_id(&self) -> &Option<ObjectId>;

    // 链接对象列表
    fn ref_objs(&self) -> &Option<Vec<ObjectLink>>;

    // 前一个版本号
    fn prev(&self) -> &Option<ObjectId>;

    // 创建时的 BTC Hash
    fn create_timestamp(&self) -> &Option<HashValue>;

    // 创建时间戳，如果不存在，则返回0
    fn create_time(&self) -> u64;

    // 过期时间戳
    fn expired_time(&self) -> Option<u64>;
}

/// 有权对象，可能是PublicKey::Single 或 PublicKey::MN
pub trait PublicKeyObjectDesc {
    fn public_key_ref(&self) -> Option<PublicKeyRef>;
}

/// 单公钥有权对象，明确用了PublicKey::Single类型
/// 实现了该Trait的对象一定同时实现了PublicKeyObjectDesc
pub trait SingleKeyObjectDesc: PublicKeyObjectDesc {
    fn public_key(&self) -> &PublicKey;
}

/// 多公钥有权对象，明确用了PublicKey::MN类型
/// 实现了该Trait的对象一定同时实现了PublicKeyObjectDesc
pub trait MNKeyObjectDesc: PublicKeyObjectDesc {
    fn mn_public_key(&self) -> &MNPublicKey;
}

/// 有主对象
pub trait OwnerObjectDesc {
    fn owner(&self) -> &Option<ObjectId>;
}

/// 有作者对象
pub trait AuthorObjectDesc {
    fn author(&self) -> &Option<ObjectId>;
}

/// 有区域对象
pub trait AreaObjectDesc {
    fn area(&self) -> &Option<Area>;
}

// obj_flags: u16
// ========
// * 前5个bits是用来指示编码状态，不计入hash计算。（计算时永远填0）
// * 剩下的11bits用来标识desc header
//
// 段编码标志位
// --------
// 0:  是否加密 crypto（现在未定义加密结构，一定填0)
// 1:  是否包含 mut_body
// 2:  是否包含 desc_signs
// 3:  是否包含 body_signs
// 4:  是否包含 nonce
//
// ObjectDesc编码标志位
// --------
// 5:  是否包含 dec_id
// 6:  是否包含 ref_objecs
// 7:  是否包含 prev
// 8:  是否包含 create_timestamp
// 9:  是否包含 create_time
// 10: 是否包含 expired_time
//
// OwnerObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc 标志位
// ---------
// 11: 是否包含 owner
// 12: 是否包含 area
// 13: 是否包含 author
// 14: 是否包含 public_key
// 15: 是否包含扩展字段
pub const OBJECT_FLAG_CTYPTO: u16 = 0x01;
pub const OBJECT_FLAG_MUT_BODY: u16 = 0x01 << 1;
pub const OBJECT_FLAG_DESC_SIGNS: u16 = 0x01 << 2;
pub const OBJECT_FLAG_BODY_SIGNS: u16 = 0x01 << 3;
pub const OBJECT_FLAG_NONCE: u16 = 0x01 << 4;

pub const OBJECT_FLAG_DESC_ID: u16 = 0x01 << 5;
pub const OBJECT_FLAG_REF_OBJECTS: u16 = 0x01 << 6;
pub const OBJECT_FLAG_PREV: u16 = 0x01 << 7;
pub const OBJECT_FLAG_CREATE_TIMESTAMP: u16 = 0x01 << 8;
pub const OBJECT_FLAG_CREATE_TIME: u16 = 0x01 << 9;
pub const OBJECT_FLAG_EXPIRED_TIME: u16 = 0x01 << 10;

pub const OBJECT_FLAG_OWNER: u16 = 0x01 << 11;
pub const OBJECT_FLAG_AREA: u16 = 0x01 << 12;
pub const OBJECT_FLAG_AUTHOR: u16 = 0x01 << 13;
pub const OBJECT_FLAG_PUBLIC_KEY: u16 = 0x01 << 14;

// 是否包含扩展字段，预留的非DescContent部分的扩展，包括一个u16长度+对应的content
pub const OBJECT_FLAG_EXT: u16 = 0x01 << 15;

// 左闭右闭 区间定义
pub const OBJECT_TYPE_ANY: u16 = 0;
pub const OBJECT_TYPE_STANDARD_START: u16 = 1;
pub const OBJECT_TYPE_STANDARD_END: u16 = 16;
pub const OBJECT_TYPE_CORE_START: u16 = 17;
pub const OBJECT_TYPE_CORE_END: u16 = 32767;
pub const OBJECT_TYPE_DECAPP_START: u16 = 32768;
pub const OBJECT_TYPE_DECAPP_END: u16 = 65535;

pub const OBJECT_PUBLIC_KEY_NONE: u8 = 0x00;
pub const OBJECT_PUBLIC_KEY_SINGLE: u8 = 0x01;
pub const OBJECT_PUBLIC_KEY_MN: u8 = 0x02;

pub const OBJECT_BODY_FLAG_PREV: u8 = 0x01;
pub const OBJECT_BODY_FLAG_USER_DATA: u8 = 0x01 << 1;
// 是否包含扩展字段，格式和desc一致
pub const OBJECT_BODY_FLAG_EXT: u8 = 0x01 << 2;

#[derive(Clone, Debug)]
pub struct NamedObjectBodyContext {
    body_content_cached_size: Option<usize>,
}

impl NamedObjectBodyContext {
    pub fn new() -> Self {
        Self {
            body_content_cached_size: None,
        }
    }

    pub fn cache_body_content_size(&mut self, size: usize) -> &mut Self {
        assert!(self.body_content_cached_size.is_none());
        self.body_content_cached_size = Some(size);
        self
    }

    pub fn get_body_content_cached_size(&self) -> usize {
        self.body_content_cached_size.unwrap()
    }
}

#[derive(Clone, Debug)]
pub struct NamedObjectContext {
    obj_type: u16,
    obj_flags: u16,
    obj_type_code: ObjectTypeCode,

    // DescContent缓存的大小
    desc_content_cached_size: Option<u16>,

    body_context: NamedObjectBodyContext,
}

impl NamedObjectContext {
    pub fn new(obj_type: u16, obj_flags: u16) -> Self {
        let obj_type_code = obj_type.into();
        Self {
            obj_type,
            obj_flags,
            obj_type_code,
            desc_content_cached_size: None,
            body_context: NamedObjectBodyContext::new(),
        }
    }

    pub fn obj_type_code(&self) -> ObjectTypeCode {
        self.obj_type_code.clone()
    }

    pub fn obj_type(&self) -> u16 {
        self.obj_type
    }

    pub fn obj_flags(&self) -> u16 {
        self.obj_flags
    }

    pub fn is_standard_object(&self) -> bool {
        self.obj_type <= 16u16
    }

    pub fn is_core_object(&self) -> bool {
        self.obj_type >= OBJECT_TYPE_CORE_START && self.obj_type <= OBJECT_TYPE_CORE_END
    }

    pub fn is_decapp_object(&self) -> bool {
        self.obj_type >= OBJECT_TYPE_DECAPP_START && self.obj_type <= OBJECT_TYPE_DECAPP_END
    }

    //
    // common
    //

    pub fn with_crypto(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_CTYPTO;
        self
    }

    pub fn has_crypto(&self) -> bool {
        self.has_flag(OBJECT_FLAG_CTYPTO)
    }

    pub fn with_mut_body(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_MUT_BODY;
        self
    }

    pub fn has_mut_body(&self) -> bool {
        self.has_flag(OBJECT_FLAG_MUT_BODY)
    }

    pub fn with_desc_signs(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_DESC_SIGNS;
        self
    }

    pub fn has_desc_signs(&self) -> bool {
        self.has_flag(OBJECT_FLAG_DESC_SIGNS)
    }

    pub fn with_body_signs(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_BODY_SIGNS;
        self
    }

    pub fn has_body_signs(&self) -> bool {
        self.has_flag(OBJECT_FLAG_BODY_SIGNS)
    }

    pub fn with_nonce(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_NONCE;
        self
    }

    pub fn has_nonce(&self) -> bool {
        self.has_flag(OBJECT_FLAG_NONCE)
    }

    //
    // ObjectDesc
    //

    pub fn with_dec_id(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_DESC_ID;
        self
    }

    pub fn has_dec_id(&self) -> bool {
        self.has_flag(OBJECT_FLAG_DESC_ID)
    }

    pub fn with_ref_objects(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_REF_OBJECTS;
        self
    }

    pub fn has_ref_objects(&self) -> bool {
        self.has_flag(OBJECT_FLAG_REF_OBJECTS)
    }

    pub fn with_prev(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_PREV;
        self
    }

    pub fn has_prev(&self) -> bool {
        self.has_flag(OBJECT_FLAG_PREV)
    }

    pub fn with_create_timestamp(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_CREATE_TIMESTAMP;
        self
    }

    pub fn has_create_time_stamp(&self) -> bool {
        self.has_flag(OBJECT_FLAG_CREATE_TIMESTAMP)
    }

    pub fn with_create_time(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_CREATE_TIME;
        self
    }

    pub fn has_create_time(&self) -> bool {
        self.has_flag(OBJECT_FLAG_CREATE_TIME)
    }

    pub fn with_expired_time(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_EXPIRED_TIME;
        self
    }

    pub fn has_expired_time(&self) -> bool {
        self.has_flag(OBJECT_FLAG_EXPIRED_TIME)
    }

    //
    // OwnderObjectDesc/AreaObjectDesc/AuthorObjectDesc/PublicKeyObjectDesc
    //

    pub fn with_owner(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_OWNER;
        self
    }

    pub fn has_owner(&self) -> bool {
        self.has_flag(OBJECT_FLAG_OWNER)
    }

    pub fn with_area(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_AREA;
        self
    }

    pub fn has_area(&self) -> bool {
        self.has_flag(OBJECT_FLAG_AREA)
    }

    pub fn with_public_key(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_PUBLIC_KEY;
        self
    }

    pub fn has_public_key(&self) -> bool {
        self.has_flag(OBJECT_FLAG_PUBLIC_KEY)
    }

    pub fn with_author(&mut self) -> &mut Self {
        self.obj_flags = self.obj_flags | OBJECT_FLAG_AUTHOR;
        self
    }

    pub fn has_author(&self) -> bool {
        self.has_flag(OBJECT_FLAG_AUTHOR)
    }

    pub fn has_ext(&self) -> bool {
        self.has_flag(OBJECT_FLAG_EXT)
    }

    // desc_content的缓存大小
    pub fn cache_desc_content_size(&mut self, size: u16) -> &mut Self {
        assert!(self.desc_content_cached_size.is_none());
        self.desc_content_cached_size = Some(size);
        self
    }

    pub fn get_desc_content_cached_size(&self) -> u16 {
        self.desc_content_cached_size.unwrap()
    }

    // body_context
    pub fn body_context(&self) -> &NamedObjectBodyContext {
        &self.body_context
    }

    pub fn mut_body_context(&mut self) -> &mut NamedObjectBodyContext {
        &mut self.body_context
    }

    // inner
    fn has_flag(&self, flag_pos: u16) -> bool {
        (self.obj_flags & flag_pos) == flag_pos
    }
}

impl RawEncode for NamedObjectContext {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(u16::raw_bytes().unwrap() * 2)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let buf = self.obj_type.raw_encode(buf, purpose).map_err(|e| {
            log::error!("NamedObjectContext::raw_encode/obj_type error:{}", e);
            e
        })?;

        let buf = self.obj_flags.raw_encode(buf, purpose).map_err(|e| {
            log::error!("NamedObjectContext::raw_encode/obj_flags error:{}", e);
            e
        })?;

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for NamedObjectContext {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (obj_type, buf) = u16::raw_decode(buf).map_err(|e| {
            log::error!("NamedObjectContext::raw_decode/obj_type error:{}", e);
            e
        })?;

        let (obj_flags, buf) = u16::raw_decode(buf).map_err(|e| {
            log::error!("NamedObjectContext::raw_decode/obj_flags error:{}", e);
            e
        })?;

        Ok((NamedObjectContext::new(obj_type, obj_flags), buf))
    }
}

#[derive(Clone)]
pub struct ObjectMutBody<B, O>
where
    O: ObjectType,
    B: BodyContent,
{
    prev_version: Option<HashValue>, // 上个版本的MutBody Hash
    update_time: u64,                // 时间戳
    content: B,                      // 根据不同的类型，可以有不同的MutBody
    user_data: Option<Vec<u8>>,      // 可以嵌入任意数据。（比如json?)
    obj_type: Option<PhantomData<O>>,
}

impl<B, O> std::fmt::Debug for ObjectMutBody<B, O>
where
    B: std::fmt::Debug + BodyContent,
    O: ObjectType,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "ObjectMutBody:{{ prev_version:{:?}, update_time:{}, version={}, format={}, content:{:?}, user_data: ... }}",
            self.prev_version, self.update_time, self.content.version(), self.content.format(), self.content,
        )
    }
}

#[derive(Clone)]
pub struct ObjectMutBodyBuilder<B, O>
where
    O: ObjectType,
    B: BodyContent,
{
    prev_version: Option<HashValue>,
    update_time: u64,
    content: B,
    user_data: Option<Vec<u8>>,
    obj_type: Option<PhantomData<O>>,
}

impl<B, O> ObjectMutBodyBuilder<B, O>
where
    B: BodyContent,
    O: ObjectType,
{
    pub fn new(content: B) -> Self {
        Self {
            prev_version: None,
            update_time: bucky_time_now(),
            content,
            user_data: None,
            obj_type: None,
        }
    }

    pub fn update_time(mut self, value: u64) -> Self {
        self.update_time = value;
        self
    }

    pub fn option_update_time(mut self, value: Option<u64>) -> Self {
        if value.is_some() {
            self.update_time = value.unwrap();
        }

        self
    }

    pub fn prev_version(mut self, value: HashValue) -> Self {
        self.prev_version = Some(value);
        self
    }

    pub fn option_prev_version(mut self, value: Option<HashValue>) -> Self {
        self.prev_version = value;
        self
    }

    pub fn user_data(mut self, value: Vec<u8>) -> Self {
        self.user_data = Some(value);
        self
    }

    pub fn option_user_data(mut self, value: Option<Vec<u8>>) -> Self {
        self.user_data = value;
        self
    }

    pub fn build(self) -> ObjectMutBody<B, O> {
        ObjectMutBody::<B, O> {
            prev_version: self.prev_version,
            update_time: self.update_time,
            content: self.content,
            user_data: self.user_data,
            obj_type: self.obj_type,
        }
    }
}

impl<B, O> ObjectMutBody<B, O>
where
    B: BodyContent,
    O: ObjectType,
{
    //pub fn new(content: B) -> ObjectMutBodyBuilder<B, O> {
    //    ObjectMutBodyBuilder::<B, O>::new(content)
    //}

    // 只读接口

    pub fn prev_version(&self) -> &Option<HashValue> {
        &self.prev_version
    }

    pub fn update_time(&self) -> u64 {
        self.update_time
    }

    pub fn content(&self) -> &B {
        &self.content
    }

    pub fn into_content(self) -> B {
        self.content
    }

    pub fn user_data(&self) -> &Option<Vec<u8>> {
        &self.user_data
    }

    // 编辑接口

    pub fn set_update_time(&mut self, value: u64) {
        self.update_time = value;
    }

    // 更新时间，并且确保大于旧时间
    pub fn increase_update_time(&mut self, mut value: u64) {
        if value < self.update_time {
            warn!(
                "object body new time is older than current time! now={}, cur={}",
                value, self.update_time
            );
            value = self.update_time + 1;
        }

        self.set_update_time(value);
    }

    pub fn content_mut(&mut self) -> &mut B {
        &mut self.content
    }

    pub fn set_userdata(&mut self, user_data: &[u8]) {
        let buf = Vec::from(user_data);
        self.user_data = Some(buf);
        self.increase_update_time(bucky_time_now());
    }

    /// 拆解Body，Move出内部数据
    /// ===
    /// * content: O::ContentType,
    /// * update_time: u64,
    /// * prev_version: Option<HashValue>,
    /// * user_data: Option<Vec<u8>>,
    pub fn split(self) -> (B, u64, Option<HashValue>, Option<Vec<u8>>) {
        let content = self.content;
        let update_time = self.update_time;
        let prev_version = self.prev_version;
        let user_data = self.user_data;
        (content, update_time, prev_version, user_data)
    }
}

impl<B, O> ObjectMutBody<B, O>
where
    B: RawEncode + BodyContent,
    O: ObjectType,
{
    pub fn calculate_hash(&self) -> BuckyResult<HashValue> {
        let hash = self.raw_hash_value().map_err(|e| {
            log::error!(
                "ObjectMutBody<B, O>::calculate_hash/raw_hash_value error:{}",
                e
            );
            e
        })?;

        Ok(hash)
    }
}

impl<'de, B, O> Default for ObjectMutBody<B, O>
where
    B: RawEncode + RawDecode<'de> + Default + BodyContent,
    O: ObjectType,
{
    fn default() -> Self {
        Self {
            prev_version: None,
            update_time: 0,
            content: B::default(),
            user_data: None,
            obj_type: None,
        }
    }
}

impl<'de, B, O> RawEncodeWithContext<NamedObjectBodyContext> for ObjectMutBody<B, O>
where
    B: RawEncode + BodyContent,
    O: ObjectType,
{
    fn raw_measure_with_context(
        &self,
        ctx: &mut NamedObjectBodyContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        // body_flags
        let mut size = u8::raw_bytes().unwrap();

        // prev_version
        if self.prev_version.is_some() {
            size = size
                + self
                    .prev_version
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!("ObjectMutBody<B, O>::raw_measure/prev_version error:{}", e);
                        e
                    })?;
        }

        // update_time
        size += u64::raw_bytes().unwrap();

        // verison+format
        size += u16::raw_bytes().unwrap();

        // content,包含usize+content
        let body_size = self.content.raw_measure(purpose).map_err(|e| {
            log::error!("ObjectMutBody<B, O>::raw_measure/content error:{}", e);
            e
        })?;
        size += USize(body_size).raw_measure(purpose)?;
        size += body_size;

        // 缓存body_size
        ctx.cache_body_content_size(body_size);

        // user_data
        let ud = self.user_data.as_ref();
        let ud_size = if ud.is_some() {
            let ud = ud.unwrap();
            let len = ud.len();
            u64::raw_bytes().unwrap() + len
        } else {
            0usize
        };

        size = size + ud_size;

        Ok(size)
    }

    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        ctx: &mut NamedObjectBodyContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        /*
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            let message = format!("[raw_encode] not enough buffer for ObjectMutBody， obj_type:{}, obj_type_code:{:?}", O::obj_type(), O::obj_type_code());
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, message));
        }
        */

        let ud = self.user_data.as_ref();

        // body_flags
        let mut body_flags = 0u8;
        if self.prev_version().is_some() {
            body_flags |= OBJECT_BODY_FLAG_PREV;
        }
        if ud.is_some() {
            body_flags |= OBJECT_BODY_FLAG_USER_DATA;
        }

        // 默认都添加版本信息，不再占用flags字段
        let buf = body_flags.raw_encode(buf, purpose).map_err(|e| {
            log::error!("ObjectMutBody<B, O>::raw_encode/body_flags error:{}", e);
            e
        })?;

        // prev_version
        let buf = if self.prev_version().is_some() {
            let buf = self
                .prev_version
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!("ObjectMutBody<B, O>::raw_encode/prev_version error:{}", e);
                    e
                })?;
            buf
        } else {
            buf
        };

        // update_time
        let buf = self.update_time.raw_encode(buf, purpose).map_err(|e| {
            log::error!("ObjectMutBody<B, O>::raw_encode/update_time error:{}", e);
            e
        })?;

        // version+format
        let buf = self
            .content
            .version()
            .raw_encode(buf, purpose)
            .map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/version error:{}", e);
                e
            })?;
        let buf = self
            .content
            .format()
            .raw_encode(buf, purpose)
            .map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/version error:{}", e);
                e
            })?;

        // content,包含usize+content
        let buf = {
            let body_size = ctx.get_body_content_cached_size();

            let buf = USize(body_size).raw_encode(buf, purpose).map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/content_usize error:{}", e);
                e
            })?;

            // 对bodycontent编码，采用精确大小的buf
            let body_buf = &mut buf[..body_size];
            let left_buf = self.content.raw_encode(body_buf, purpose).map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/content error:{}", e);
                e
            })?;

            if left_buf.len() != 0 {
                warn!("encode body content by remaining buf is not empty! obj_type={}, body_size={}, remaining={}", O::obj_type(), body_size, left_buf.len());
                // assert!(left_buf.len() == 0);
            }

            &mut buf[body_size..]
        };

        // user_data
        let buf = if ud.is_some() {
            let ud = ud.unwrap();
            let len = ud.len();
            let buf = (len as u64).raw_encode(buf, purpose).map_err(|e| {
                log::error!("ObjectMutBody<B, O>::raw_encode/user_data len error:{}", e);
                e
            })?;

            buf[..len].copy_from_slice(ud.as_slice());
            &mut buf[len..]
        } else {
            buf
        };

        Ok(buf)
    }
}

impl<'de, B, O> RawDecodeWithContext<'de, &NamedObjectBodyContext> for ObjectMutBody<B, O>
where
    B: RawDecode<'de> + BodyContent,
    O: ObjectType,
{
    fn raw_decode_with_context(
        buf: &'de [u8],
        _ctx: &NamedObjectBodyContext,
    ) -> BuckyResult<(Self, &'de [u8])> {
        // body_flags
        let (body_flags, buf) = u8::raw_decode(buf).map_err(|e| {
            let msg = format!("ObjectMutBody<B, O>::raw_decode/body_flags error:{}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // prev_version
        let (prev_version, buf) = if (body_flags & OBJECT_BODY_FLAG_PREV) == OBJECT_BODY_FLAG_PREV {
            let (prev_version, buf) = HashValue::raw_decode(buf).map_err(|e| {
                let msg = format!("ObjectMutBody<B, O>::raw_decode/prev_version error:{}", e);
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;
            (Some(prev_version), buf)
        } else {
            (None, buf)
        };

        // update_time
        let (update_time, buf) = u64::raw_decode(buf).map_err(|e| {
            let msg = format!(
                "ObjectMutBody<B, O>::raw_decode/update_time error:{}, body={}",
                e,
                B::debug_info()
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // 这里尝试读取是否存在ext扩展字段，如果存在那么为了向前兼容要跳过
        let buf = if (body_flags & OBJECT_BODY_FLAG_EXT) == OBJECT_BODY_FLAG_EXT {
            let (len, buf) = u16::raw_decode(buf).map_err(|e| {
                let msg = format!(
                    "ObjectMutBody<B, O>::raw_decode/ext error:{}, body={}",
                    e,
                    B::debug_info()
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;
            warn!(
                "read unknown body ext content! len={}, body={}",
                len,
                B::debug_info()
            );

            if len as usize > buf.len() {
                let msg = format!("read unknown body ext content but extend buffer limit, body={}, len={}, buf={}", B::debug_info(), len, buf.len());
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
            }

            // 跳过指定的长度
            &buf[len as usize..]
        } else {
            buf
        };

        // version
        let (version, buf) = u8::raw_decode(buf).map_err(|e| {
            let msg = format!("ObjectMutBody<B, O>::raw_decode/version error:{}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // format
        let (format, buf) = u8::raw_decode(buf).map_err(|e| {
            let msg = format!("ObjectMutBody<B, O>::raw_decode/format error:{}", e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })?;

        // 对于BodyContent，使用带option的解码，用以兼容老版本的解码
        let opt = RawDecodeOption { version, format };

        // body content
        let (content, buf) = {
            let (body_size, buf) = USize::raw_decode(buf).map_err(|e| {
                let msg = format!(
                    "ObjectMutBody<B, O>::raw_decode/content_usize error:{}, body={}",
                    e,
                    B::debug_info()
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            let body_size = body_size.value();
            if buf.len() < body_size {
                let msg = format!(
                    "invalid body content buffer size: expect={}, buf={}, body={}",
                    body_size,
                    buf.len(),
                    B::debug_info(),
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
            }

            // 使用精确大小的buffer解码
            let body_buf = &buf[..body_size];
            let (content, left_buf) = B::raw_decode_with_option(&body_buf, &opt).map_err(|e| {
                let msg = format!(
                    "ObjectMutBody<B, O>::raw_decode/content error:{}, body={}",
                    e,
                    B::debug_info()
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            if left_buf.len() != 0 {
                warn!("decode body content by remaining buf is not empty! obj_type={}, body_size={}, remaining={}", O::obj_type(), body_size, left_buf.len());
                // assert!(left_buf.len() == 0);
            }

            (content, &buf[body_size..])
        };

        // user_data
        let (user_data, buf) = if (body_flags & OBJECT_BODY_FLAG_USER_DATA)
            == OBJECT_BODY_FLAG_USER_DATA
        {
            let (len, buf) = u64::raw_decode(buf).map_err(|e| {
                let msg = format!(
                    "ObjectMutBody<B, O>::raw_decode/user_data len error:{}, body={}",
                    e,
                    B::debug_info()
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::InvalidData, msg)
            })?;

            let bytes = len as usize;
            if bytes > buf.len() {
                let msg = format!("ObjectMutBody<B, O>::raw_decode/user_data len overflow: body={}, len={}, left={}",
                        B::debug_info(),
                        len,
                        buf.len()
                    );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }

            let mut user_data = vec![0u8; bytes];
            user_data.copy_from_slice(&buf[..bytes]);
            (Some(user_data), &buf[bytes..])
        } else {
            (None, buf)
        };

        Ok((
            Self {
                prev_version,
                update_time,
                content,
                user_data,
                obj_type: None,
            },
            buf,
        ))
    }
}

impl<'de, B, O> RawEncode for ObjectMutBody<B, O>
where
    B: RawEncode + BodyContent,
    O: ObjectType,
{
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        unimplemented!();
    }

    fn raw_encode<'a>(
        &self,
        _buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        unimplemented!();
    }

    fn raw_hash_encode(&self) -> BuckyResult<Vec<u8>> {
        let mut ctx = NamedObjectBodyContext::new();
        let size = self.raw_measure_with_context(&mut ctx, &Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("ObjectMutBody<B, O>::rraw_measure_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code());
            e
        })?;

        let mut buf = vec![0u8; size];
        let left_buf = self.raw_encode_with_context(&mut buf, &mut ctx, &Some(RawEncodePurpose::Hash)).map_err(|e|{
            error!("ObjectMutBody<B, O>::raw_encode/self.raw_encode_with_context error:{}, obj_type:{}, obj_type_code:{:?}", e, O::obj_type(), O::obj_type_code());
            e
        })?;

        if left_buf.len() != 0 {
            warn!("encode body content by remaining buf is not empty! obj_type={}, body_size={}, remaining={}", O::obj_type(), size, left_buf.len());
            // assert!(left_buf.len() == 0);
        }

        Ok(buf)
    }
}

impl<'de, B, O> RawDecode<'de> for ObjectMutBody<B, O>
where
    B: RawDecode<'de> + BodyContent,
    O: ObjectType,
{
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let ctx = NamedObjectBodyContext::new();
        Self::raw_decode_with_context(buf, &ctx)
    }
}

#[derive(Clone, Debug)]
pub struct ObjectSigns {
    // 对Desc部分的签名，可以是多个，sign结构有的时候需要说明是“谁的签名”
    // 表示对Desc内容的认可。
    desc_signs: Option<Vec<Signature>>,

    // 对MutBody部分的签名，可以是多个。依赖MutBody的稳定编码
    body_signs: Option<Vec<Signature>>,
}

pub struct ObjectSignsHelper;

impl ObjectSignsHelper {
    pub fn set_sign(list: &mut Option<Vec<Signature>>, sign: Signature) {
        if list.is_none() {
            *list = Some(vec![]);
        }

        let list = list.as_mut().unwrap();
        list.clear();
        list.push(sign);
    }

    pub fn push_sign(list: &mut Option<Vec<Signature>>, sign: Signature) {
        if list.is_none() {
            *list = Some(vec![]);
        }

        let list = list.as_mut().unwrap();

        // 相同source的，只保留已有的签名，忽略时间和签名内容
        if let Some(cur) = list.iter_mut().find(|v| v.compare_source(&sign)) {
            if sign.sign_time() > cur.sign_time() {
                info!("desc sign with same source will update! new={:?}, cur={:?}", sign, cur);
                *cur = sign;
            } else{
                warn!("desc sign with same source already exists! sign={:?}", sign);
            }
        } else {
            list.push(sign);
        }
    }

    pub fn latest_sign_time(list: &Option<Vec<Signature>>) -> u64 {
        let mut ret = 0;
        if let Some(signs) = list.as_ref() {
            for sign in signs {
                if ret < sign.sign_time() {
                    ret = sign.sign_time();
                }
            }
        }
        ret
    }

    // FIXME dest和src合并，相同source签名是保留dest的，还是时间戳比较老的？
    pub fn merge(dest: &mut Option<Vec<Signature>>, src: &Option<Vec<Signature>>) -> usize {
        match src {
            Some(src) => {
                match dest {
                    Some(dest) => {
                        let mut ret = 0;
                        for src_item in src {
                            match dest.iter_mut().find(|s| s.compare_source(src_item)) {
                                Some(dest_item) => {
                                    if src_item.sign_time() > dest_item.sign_time() {
                                        *dest_item = src_item.clone();
                                    }
                                }
                                None => {
                                    dest.push(src_item.clone());
                                    ret += 1;
                                }

                            }
                        }
                        ret
                    }
                    None => {
                        // src里面的签名也需要确保去重
                        let mut ret = 0;
                        let mut dest_list: Vec<Signature> = Vec::new();
                        for src_item in src {
                            match dest_list.iter_mut().find(|s| s.compare_source(src_item)) {
                                Some(dest_item) => {
                                    if src_item.sign_time() > dest_item.sign_time() {
                                        *dest_item = src_item.clone();
                                    }
                                }
                                None => {
                                    dest_list.push(src_item.clone());
                                    ret += 1;
                                }
                            }
                        }

                        if !dest_list.is_empty() {
                            *dest = Some(dest_list);
                        }

                        ret
                    }
                }
            }
            None => 0,
        }
    }
}

#[derive(Clone)]
pub struct ObjectSignsBuilder {
    desc_signs: Option<Vec<Signature>>,
    body_signs: Option<Vec<Signature>>,
}

impl ObjectSignsBuilder {
    pub fn new() -> Self {
        Self {
            desc_signs: None,
            body_signs: None,
        }
    }

    pub fn set_desc_sign(mut self, sign: Signature) -> Self {
        ObjectSignsHelper::set_sign(&mut self.desc_signs, sign);
        self
    }

    pub fn set_body_sign(mut self, sign: Signature) -> Self {
        ObjectSignsHelper::set_sign(&mut self.body_signs, sign);
        self
    }

    pub fn push_desc_sign(mut self, sign: Signature) -> Self {
        ObjectSignsHelper::push_sign(&mut self.desc_signs, sign);
        self
    }

    pub fn push_body_sign(mut self, sign: Signature) -> Self {
        ObjectSignsHelper::push_sign(&mut self.body_signs, sign);
        self
    }

    pub fn build(self) -> ObjectSigns {
        ObjectSigns {
            desc_signs: self.desc_signs,
            body_signs: self.body_signs,
        }
    }
}

impl ObjectSigns {
    pub fn new() -> ObjectSignsBuilder {
        ObjectSignsBuilder::new()
    }

    pub fn is_desc_signs_empty(&self) -> bool {
        if let Some(signs) = &self.desc_signs {
            signs.len() == 0
        } else {
            true
        }
    }

    pub fn is_body_signs_empty(&self) -> bool {
        if let Some(signs) = &self.body_signs {
            signs.len() == 0
        } else {
            true
        }
    }

    pub fn is_empty(&self) -> bool {
        self.is_desc_signs_empty() && self.is_body_signs_empty()
    }

    pub fn desc_signs(&self) -> Option<&Vec<Signature>> {
        self.desc_signs.as_ref()
    }

    pub fn body_signs(&self) -> Option<&Vec<Signature>> {
        self.body_signs.as_ref()
    }

    pub fn set_desc_sign(&mut self, sign: Signature) {
        ObjectSignsHelper::set_sign(&mut self.desc_signs, sign)
    }

    pub fn set_body_sign(&mut self, sign: Signature) {
        ObjectSignsHelper::set_sign(&mut self.body_signs, sign)
    }

    pub fn push_desc_sign(&mut self, sign: Signature) {
        ObjectSignsHelper::push_sign(&mut self.desc_signs, sign)
    }

    pub fn push_body_sign(&mut self, sign: Signature) {
        ObjectSignsHelper::push_sign(&mut self.body_signs, sign)
    }

    pub fn clear_desc_signs(&mut self) {
        self.desc_signs.clear();
    }

    pub fn clear_body_signs(&mut self) {
        self.body_signs.clear();
    }

    pub fn latest_body_sign_time(&self) -> u64 {
        ObjectSignsHelper::latest_sign_time(&self.body_signs)
    }

    pub fn latest_desc_sign_time(&self) -> u64 {
        ObjectSignsHelper::latest_sign_time(&self.desc_signs)
    }

    pub fn latest_sign_time(&self) -> u64 {
        std::cmp::max(self.latest_desc_sign_time(), self.latest_body_sign_time())
    }

    pub fn merge(&mut self, other: &ObjectSigns) -> usize {
        ObjectSignsHelper::merge(&mut self.desc_signs, &other.desc_signs)
            + ObjectSignsHelper::merge(&mut self.body_signs, &other.body_signs)
    }

    pub fn merge_ex(&mut self, other: &ObjectSigns, desc: bool, body: bool) -> usize {
        let mut ret = 0;
        if desc {
            ret += ObjectSignsHelper::merge(&mut self.desc_signs, &other.desc_signs);
        }
        if body {
            ret += ObjectSignsHelper::merge(&mut self.body_signs, &other.body_signs);
        }

        ret
    }
}

impl Default for ObjectSigns {
    fn default() -> Self {
        Self {
            desc_signs: None,
            body_signs: None,
        }
    }
}

impl RawEncodeWithContext<NamedObjectContext> for ObjectSigns {
    fn raw_measure_with_context(
        &self,
        ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<usize> {
        let mut size = 0;

        if self.desc_signs.is_some() {
            ctx.with_desc_signs();
            assert!(ctx.has_desc_signs());
            size = size
                + self
                    .desc_signs
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "ObjectSigns::raw_measure_with_context/desc_signs error:{}",
                            e
                        );
                        e
                    })?;
        }

        if self.body_signs.is_some() {
            ctx.with_body_signs();
            assert!(ctx.has_body_signs());
            size = size
                + self
                    .body_signs
                    .as_ref()
                    .unwrap()
                    .raw_measure(purpose)
                    .map_err(|e| {
                        log::error!(
                            "ObjectSigns::raw_measure_with_context/body_signs error:{}",
                            e
                        );
                        e
                    })?;
        }

        Ok(size)
    }

    // 调用之前，必须已经被正确measure过了
    fn raw_encode_with_context<'a>(
        &self,
        buf: &'a mut [u8],
        ctx: &mut NamedObjectContext,
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        /*
        let size = self.raw_measure_with_context(ctx, purpose).unwrap();
        if buf.len() < size {
            let msg = format!(
                "not enough buffer for encode ObjectSigns, except={}, got={}",
                size,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        */

        let buf = if self.desc_signs.is_some() {
            assert!(ctx.has_desc_signs());
            self.desc_signs
                .as_ref()
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "ObjectSigns::raw_encode_with_context/desc_signs error:{}",
                        e
                    );
                    e
                })?
        } else {
            buf
        };

        let buf = if self.body_signs.is_some() {
            assert!(ctx.has_body_signs());
            self.body_signs
                .as_ref()
                .unwrap()
                .raw_encode(buf, purpose)
                .map_err(|e| {
                    log::error!(
                        "ObjectSigns::raw_encode_with_context/body_signs error:{}",
                        e
                    );
                    e
                })?
        } else {
            buf
        };

        Ok(buf)
    }
}

impl<'de> RawDecodeWithContext<'de, &NamedObjectContext> for ObjectSigns {
    fn raw_decode_with_context(
        buf: &'de [u8],
        ctx: &NamedObjectContext,
    ) -> BuckyResult<(Self, &'de [u8])> {
        let (desc_signs, buf) = if ctx.has_desc_signs() {
            let (desc_signs, buf) = Vec::<Signature>::raw_decode(buf).map_err(|e| {
                log::error!(
                    "ObjectSigns::raw_decode_with_context/desc_signs error:{}",
                    e
                );
                e
            })?;
            (Some(desc_signs), buf)
        } else {
            (None, buf)
        };

        let (body_signs, buf) = if ctx.has_body_signs() {
            let (body_signs, buf) = Vec::<Signature>::raw_decode(buf).map_err(|e| {
                log::error!(
                    "ObjectSigns::raw_decode_with_context/body_signs error:{}",
                    e
                );
                e
            })?;
            (Some(body_signs), buf)
        } else {
            (None, buf)
        };

        Ok((
            Self {
                desc_signs,
                body_signs,
            },
            buf,
        ))
    }
}

pub trait NamedObject<O>
where
    O: ObjectType,
    O::ContentType: BodyContent,
{
    fn obj_flags(&self) -> u16;

    fn desc(&self) -> &O::DescType;

    fn desc_mut(&mut self) -> &mut O::DescType;

    fn body(&self) -> &Option<ObjectMutBody<O::ContentType, O>>;

    fn body_expect(&self, msg: &str) -> &ObjectMutBody<O::ContentType, O> {
        let msg = format!("expect obj_type {} body failed, msg:{}", O::obj_type(), msg);
        self.body().as_ref().expect(&msg)
    }

    fn body_mut(&mut self) -> &mut Option<ObjectMutBody<O::ContentType, O>>;

    fn body_mut_expect(&mut self, msg: &str) -> &mut ObjectMutBody<O::ContentType, O> {
        let msg = format!(
            "expect obj_type {} body_mut failed, msg:{}",
            O::obj_type(),
            msg
        );
        self.body_mut().as_mut().expect(&msg)
    }

    fn signs(&self) -> &ObjectSigns;

    fn signs_mut(&mut self) -> &mut ObjectSigns;

    fn nonce(&self) -> &Option<u128>;

    // 获取body和sign部分的最新的修改时间
    fn latest_update_time(&self) -> u64 {
        let update_time = match self.body() {
            Some(body) => body.update_time(),
            None => 0_u64,
        };

        // 如果签名时间比较新，那么取签名时间
        let latest_sign_time = self.signs().latest_sign_time();

        std::cmp::max(update_time, latest_sign_time)
    }
}

// 用 base16 hex实现serde
use primitive_types::H256;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt::Formatter;

impl Serialize for ObjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ObjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RawObjectIdVisitor;
        impl<'de> Visitor<'de> for RawObjectIdVisitor {
            type Value = ObjectId;
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(formatter, "{}", "an ObjectId")
            }
            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                ObjectId::from_str(v).map_err(|err| E::custom(err.to_string()))
            }
        }
        deserializer.deserialize_str(RawObjectIdVisitor)
    }
}

impl RawDiff for ObjectId {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let data = self.as_ref();
        let r = right.as_ref();
        data.diff_measure(r)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let size = self.diff_measure(right).map_err(|e| {
            log::error!("ObjectId::diff/diff_measure error:{}", e);
            e
        })?;

        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for ObjectId",
            ));
        }

        self.as_ref().diff(right.as_ref(), buf)
    }
}

impl<'de> RawPatch<'de> for ObjectId {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let data: GenericArray<u8, U32> = self.into();
        let (data, buf) = data.patch(buf).map_err(|e| {
            log::error!("ObjectId::patch/data error:{}", e);
            e
        })?;
        Ok((ObjectId::from(data), buf))
    }
}

impl<T: ObjectType> RawDiff for NamedObjectId<T> {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let left = self.object_id();
        let right = right.object_id();
        left.diff_measure(right)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let size = self.diff_measure(right).map_err(|e| {
            log::error!("NamedObjectId::diff error:{}", e);
            e
        })?;

        if buf.len() < size {
            let message = format!(
                "[raw_encode] not enough buffer for NamedObjectId, obj_type:{}, obj_type_code:{:?}",
                T::obj_type(),
                T::obj_type_code()
            );
            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, message));
        }

        self.object_id().diff(right.object_id(), buf)
    }
}

impl<'de, T: ObjectType> RawPatch<'de> for NamedObjectId<T> {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (object_id, buf) = self.object_id().patch(buf).map_err(|e| {
            log::error!("NamedObjectId::patch error:{}", e);
            e
        })?;

        Ok((
            NamedObjectId::<T>::try_from(object_id).map_err(|e| {
                log::error!("NamedObjectId::patch/try_from error:{}", e);
                e
            })?,
            buf,
        ))
    }
}

// TODO concat_idents!目前是unstable功能
#[macro_export]
macro_rules! declare_object {
    ($name:ident) => {
        type concat_idents!($name, Type) = cyfs_base::NamedObjType<concat_idents!($name, DescContent), concat_idents!($name, BodyContent)>;
        type concat_idents!($name, Builder) = cyfs_base::NamedObjectBuilder<concat_idents!($name, DescContent), concat_idents!($name, BodyContent)>;

        type concat_idents!($name, Id) = cyfs_base::NamedObjectId<concat_idents!($name, Type)>;
        type concat_idents!($name, Desc) = cyfs_base::NamedObjectDesc<concat_idents!($name, DescContent)>;
        type $name = cyfs_base::NamedObjectBase<concat_idents!($name,Type)>;
    }
}

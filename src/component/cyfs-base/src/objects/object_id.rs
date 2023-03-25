use crate::*;

use base58::{FromBase58, ToBase58};
use generic_array::typenum::{marker_traits::Unsigned, U32};
use generic_array::GenericArray;
use primitive_types::H256;
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ObjectCategory {
    Standard,
    Core,
    DecApp,
    Data,
}

impl ToString for ObjectCategory {
    fn to_string(&self) -> String {
        (match *self {
            Self::Standard => "standard",
            Self::Core => "core",
            Self::DecApp => "dec_app",
            Self::Data => "data",
        })
        .to_owned()
    }
}

impl FromStr for ObjectCategory {
    type Err = BuckyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let ret = match value {
            "standard" => Self::Standard,
            "core" => Self::Core,
            "dec_app" => Self::DecApp,
            "data" => Self::Data,
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

/*
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
*/

impl TryFrom<Vec<u8>> for ObjectId {
    type Error = BuckyError;
    fn try_from(v: Vec<u8>) -> Result<Self, Self::Error> {
        if v.len() != 32 {
            let msg = format!(
                "ObjectId expected bytes of length {} but it was {}",
                32,
                v.len()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        let ar: [u8; 32] = v.try_into().unwrap();
        Ok(Self(GenericArray::from(ar)))
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
        ObjectId::clone_from_slice(val.as_ref()).unwrap()
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

pub const OBJECT_ID_DATA: u8 = 0b_00000000;
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

pub enum ObjectIdInfo<'a> {
    Data(&'a [u8]),
    Standard(StandardObjectIdInfo),
    Core(CoreObjectIdInfo),
    DecApp(DecAppObjectIdInfo),
}

impl<'a> ObjectIdInfo<'a> {
    pub fn area(&self) -> &Option<Area> {
        match self {
            Self::Data(_) => &None,
            Self::Standard(v) => &v.area,
            Self::Core(v) => &v.area,
            Self::DecApp(v) => &v.area,
        }
    }

    pub fn into_area(self) -> Option<Area> {
        match self {
            Self::Data(_) => None,
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

        if !self.t.is_standard_object() {
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

pub struct ObjectIdDataBuilder<'a> {
    data: Option<&'a [u8]>,
}

impl<'a> ObjectIdDataBuilder<'a> {
    pub fn new() -> Self {
        Self { data: None }
    }

    pub fn data(mut self, data: &'a (impl AsRef<[u8]> + ?Sized)) -> Self {
        self.data = Some(data.as_ref());
        self
    }

    pub fn build_empty() -> ObjectId {
        ObjectIdDataBuilder::new().build().unwrap()
    }

    pub fn build(self) -> BuckyResult<ObjectId> {
        let mut id = GenericArray::<u8, U32>::default();
        if let Some(data) = self.data {
            let len = data.len();
            if len > 31 {
                let msg = format!("invalid object id data len! len={}, max={}", len, 31);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }

            id.as_mut_slice()[0] = len as u8;
            id.as_mut_slice()[1..(len + 1)].copy_from_slice(data);
        }

        Ok(ObjectId::new(id))
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
            OBJECT_ID_DATA => ObjectIdInfo::Data(self.data()),
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

    pub fn clone_from_slice(slice: &[u8]) -> BuckyResult<Self> {
        if slice.len() != U32::to_usize() {
            let msg = format!("invalid buf len for ObjectId: len={}", slice.len());
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        Ok(ObjectId(GenericArray::clone_from_slice(slice)))
    }

    pub fn to_string(&self) -> String {
        self.0.as_slice().to_base58()
    }

    pub fn to_hash_value(&self) -> HashValue {
        self.0.as_slice().try_into().unwrap()
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

        Ok(Self::try_from(buf).unwrap())
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

        Ok(Self::try_from(buf).unwrap())
    }

    pub fn object_category(&self) -> ObjectCategory {
        let flags = self.as_slice()[0] >> 6;
        match flags {
            OBJECT_ID_DATA => ObjectCategory::Data,
            OBJECT_ID_STANDARD => ObjectCategory::Standard,
            OBJECT_ID_CORE => ObjectCategory::Core,
            OBJECT_ID_DEC_APP => ObjectCategory::DecApp,
            _ => {
                unreachable!();
            }
        }
    }

    pub fn is_data(&self) -> bool {
        let buf = self.as_slice();
        let flag = buf[0];
        flag >> 6 == OBJECT_ID_DATA
    }

    pub fn is_standard_object(&self) -> bool {
        let buf = self.as_slice();
        let flag = buf[0];
        flag >> 6 == OBJECT_ID_STANDARD
    }

    pub fn is_core_object(&self) -> bool {
        let buf = self.as_slice();
        let flag = buf[0];
        flag >> 6 == OBJECT_ID_CORE
    }

    pub fn is_dec_app_object(&self) -> bool {
        let buf = self.as_slice();
        let flag = buf[0];
        flag >> 6 == OBJECT_ID_DEC_APP
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

    pub fn data_len(&self) -> u8 {
        self.as_slice()[0] & 0b_00111111
    }

    pub fn data(&self) -> &[u8] {
        let len = self.data_len();
        &self.as_slice()[1..len as usize + 1]
    }

    pub fn data_as_utf8_string(&self) -> BuckyResult<&str> {
        std::str::from_utf8(self.data()).map_err(|e| {
            let msg = format!(
                "invalid object id data as utf8 string! id={}, {}",
                self.to_string(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidData, msg)
        })
    }

    pub fn data_as_utf8_string_unchecked(&self) -> &str {
        unsafe { std::str::from_utf8_unchecked(self.data()) }
    }

    pub fn is_chunk_id(&self) -> bool {
        match self.obj_type_code() {
            ObjectTypeCode::Chunk => true,
            _ => false,
        }
    }
    
    pub fn as_chunk_id(&self) -> &ChunkId {
        unsafe { std::mem::transmute::<&ObjectId, &ChunkId>(&self) }
    }

    pub fn as_named_object_id<T: ObjectType>(&self) -> &NamedObjectId<T> {
        unsafe { std::mem::transmute::<&ObjectId, &NamedObjectId<T>>(&self) }
    }

    pub fn is_default(&self) -> bool {
        self == &ObjectId::default()
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

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn test_data() {
        let data = "hello!!! first id";
        let id = ObjectIdDataBuilder::new().data(data).build().unwrap();
        assert!(id.is_data());
        assert!(!id.is_standard_object());
        assert!(!id.is_core_object());
        assert!(!id.is_dec_app_object());

        println!("len={}, {}", id.data_len(), id.to_string());

        let data2 = id.data();
        assert_eq!(data2.len(), data.as_bytes().len());
        assert_eq!(data2, data.as_bytes());
        assert_eq!(id.data_as_utf8_string_unchecked(), data);

        let id = ObjectIdDataBuilder::build_empty();
        assert!(id.is_data());
        println!("len={}, {}", id.data_len(), id.to_string());
        assert_eq!(id.data_len(), 0);
        assert_eq!(id.data().len(), 0);

        let error_data = "1234567890123456789012345678901234567890";
        let ret = ObjectIdDataBuilder::new().data(error_data).build();
        assert!(ret.is_err());

        let data = hash_data("1233".as_bytes());
        let id = ObjectIdDataBuilder::new()
            .data(&data.as_slice()[0..31])
            .build()
            .unwrap();
        println!("len={}, {}", id.data_len(), id.to_string());

        assert_eq!(id.data_len(), 31);
        assert_eq!(id.data(), &data.as_slice()[0..31]);
        id.data_as_utf8_string().unwrap_err();

        assert_eq!(id.object_category(), ObjectCategory::Data);

        match id.info() {
            ObjectIdInfo::Data(v) => {
                assert_eq!(v, &data.as_slice()[0..31]);
            }
            _ => unreachable!(),
        }

        // let s = id.data_as_utf8_string_unchecked();
        // println!("{}", s);
    }
}

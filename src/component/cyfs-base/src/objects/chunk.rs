use crate::*;

use base58::{FromBase58, ToBase58};
use generic_array::typenum::{marker_traits::Unsigned, U32};
use generic_array::GenericArray;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::{
    convert::{Into, TryFrom},
    str::FromStr,
};

// unique id in const info
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct ChunkId(GenericArray<u8, U32>);

impl Default for ChunkId {
    fn default() -> Self {
        Self(GenericArray::default())
    }
}

impl Hash for ChunkId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut buff = [0 as u8; 32];
        let _ = self.raw_encode(buff.as_mut(), &None).unwrap();
        state.write(buff.as_ref());
    }
}

impl std::fmt::Debug for ChunkId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ChunkId: {:?}", self.0.as_slice().to_base58())
    }
}

impl std::fmt::Display for ChunkId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.as_slice().to_base58())
    }
}

impl FromStr for ChunkId {
    type Err = BuckyError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        if OBJECT_ID_BASE36_RANGE.contains(&s.len()) {
            Self::from_base36(s)
        } else {
            Self::from_base58(s)
        }
    }
}

impl TryFrom<&ObjectId> for ChunkId {
    type Error = BuckyError;

    fn try_from(id: &ObjectId) -> Result<Self, Self::Error> {
        let obj_type_code = id.obj_type_code();

        if obj_type_code == ObjectTypeCode::Chunk {
            Ok(Self(id.as_ref().clone()))
        } else {
            Err(
                BuckyError::new(
                    BuckyErrorCode::InvalidParam,
                    format!("try convert from object id to named object id failed, mismatch obj_type_code, expect obj_type_code is: {}, current obj_type_code is:{}", ObjectTypeCode::Chunk.to_string(), obj_type_code.to_string())
                )
            )
        }
    }
}

impl ProtobufTransform<ChunkId> for Vec<u8> {
    fn transform(value: ChunkId) -> BuckyResult<Self> {
        Ok(Vec::from(value.0.as_slice()))
    }
}

impl ProtobufTransform<&ChunkId> for Vec<u8> {
    fn transform(value: &ChunkId) -> BuckyResult<Self> {
        Ok(Vec::from(value.0.as_slice()))
    }
}

impl ProtobufTransform<Vec<u8>> for ChunkId {
    fn transform(value: Vec<u8>) -> BuckyResult<Self> {
        if value.len() != U32::to_usize() {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidParam,
                format!(
                    "try convert from vec<u8> to chunk id failed, invalid len {}",
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

impl From<[u8; 32]> for ChunkId {
    fn from(v: [u8; 32]) -> Self {
        Self(GenericArray::from(v))
    }
}

impl From<Vec<u8>> for ChunkId {
    fn from(v: Vec<u8>) -> Self {
        let ar: [u8; 32] = v.try_into().unwrap_or_else(|v: Vec<u8>| {
            panic!(
                "ChunkId expected a Vec of length {} but it was {}",
                32,
                v.len()
            )
        });

        Self(GenericArray::from(ar))
    }
}

impl From<GenericArray<u8, U32>> for ChunkId {
    fn from(chunk_id: GenericArray<u8, U32>) -> Self {
        Self(chunk_id)
    }
}

impl From<ChunkId> for GenericArray<u8, U32> {
    fn from(hash: ChunkId) -> Self {
        hash.0
    }
}

impl AsRef<GenericArray<u8, U32>> for ChunkId {
    fn as_ref(&self) -> &GenericArray<u8, U32> {
        &self.0
    }
}

impl ChunkId {
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn obj_type_code(&self) -> Option<ObjectTypeCode> {
        Some(ObjectTypeCode::Chunk)
    }

    pub fn object_id(&self) -> ObjectId {
        ObjectId::clone_from_slice(self.as_slice()).unwrap()
    }

    pub fn as_object_id(&self) -> &ObjectId {
        unsafe { std::mem::transmute::<&ChunkId, &ObjectId>(&self) }
    }

    fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }

    pub fn to_string(&self) -> String {
        self.0.as_slice().to_base58()
    }

    pub fn to_base36(&self) -> String {
        self.0.as_slice().to_base36()
    }

    pub fn from_base58(s: &str) -> BuckyResult<Self> {
        let buf = s.from_base58().map_err(|e| {
            let msg = format!("convert base58 str to chunk id failed, str:{}, {:?}", s, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::ParseError, msg)
        })?;

        if buf.len() != U32::to_usize() {
            let msg = format!(
                "convert base58 str to chunk id failed, str's len not matched:{}, len:{}",
                s,
                buf.len()
            );
            return Err(BuckyError::new(BuckyErrorCode::ParseError, msg));
        }

        Ok(Self::from(buf))
    }

    pub fn from_base36(s: &str) -> BuckyResult<Self> {
        let buf = s.from_base36().map_err(|e| {
            let msg = format!("convert base36 str to chunk id failed, str:{}, {:?}", s, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::ParseError, msg)
        })?;

        if buf.len() != U32::to_usize() {
            let msg = format!(
                "convert base36 str to chunk id failed, str's len not matched:{}, len:{}",
                s,
                buf.len()
            );
            return Err(BuckyError::new(BuckyErrorCode::ParseError, msg));
        }

        Ok(Self::from(buf))
    }

    pub async fn calculate(data: &[u8]) -> BuckyResult<Self> {
        let hash = hash_data(data);
        Ok(ChunkId::new(&hash, data.len() as u32))
    }

    pub fn calculate_sync(data: &[u8]) -> BuckyResult<Self> {
        let hash = hash_data(data);
        Ok(ChunkId::new(&hash, data.len() as u32))
    }

    pub fn new(hash_value: &HashValue, len: u32) -> Self {
        let hash = hash_value.as_slice();

        let mut id = Self::default();
        let chunkid = id.as_mut_slice();
        chunkid[0] = 0b_01000000 | (ObjectTypeCode::Chunk.to_u16() as u8) << 4 >> 2;
        // chunkid[0] = ObjectTypeCode::Chunk.to_u16() as u8;
        unsafe {
            *(chunkid[1..5].as_mut_ptr() as *mut u32) = len;
        }
        chunkid[5..].copy_from_slice(&hash[0..27]);
        id
    }

    pub fn hash(&self) -> &[u8] {
        let chunkid = self.as_slice();
        &chunkid[5..]
    }

    pub fn len(&self) -> usize {
        let chunkid = self.as_slice();
        return unsafe { *(chunkid[1..5].as_ptr() as *const u32) } as usize;
    }
}

impl RawFixedBytes for ChunkId {
    fn raw_bytes() -> Option<usize> {
        Some(U32::to_usize())
    }
}

impl RawEncode for ChunkId {
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
                "not enough buffer for encode ChunkId, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        unsafe {
            std::ptr::copy(self.0.as_slice().as_ptr(), buf.as_mut_ptr(), bytes);
        }

        Ok(&mut buf[bytes..])
    }
}

impl<'de> RawDecode<'de> for ChunkId {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for decode ChunkId, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        let mut _id = Self::default();
        unsafe {
            std::ptr::copy(buf.as_ptr(), _id.0.as_mut_slice().as_mut_ptr(), bytes);
        }
        Ok((_id, &buf[bytes..]))
    }
}

use super::raw_diff::{RawDiff, RawPatch};

impl RawDiff for ChunkId {
    fn diff_measure(&self, right: &Self) -> BuckyResult<usize> {
        let data = self.as_ref();
        let r = right.as_ref();
        data.diff_measure(r)
    }

    fn diff<'d>(&self, right: &Self, buf: &'d mut [u8]) -> BuckyResult<&'d mut [u8]> {
        let size = self.diff_measure(right).map_err(|e| {
            log::error!("ChunkId::diff error:{}", e);
            e
        })?;
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_diff] not enough buffer for chunk_id",
            ));
        }

        self.as_ref().diff(right.as_ref(), buf)
    }
}

impl<'de> RawPatch<'de> for ChunkId {
    fn patch(self, buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let data: GenericArray<u8, U32> = self.into();
        let (data, buf) = data.patch(buf).map_err(|e| {
            log::error!("ChunkId::patch error:{}", e);
            e
        })?;
        Ok((ChunkId::from(data), buf))
    }
}

/// Chunk 存活状态机、
/// * 默认不存在
/// * 如果应用层表示感兴趣并且没有在被忽略路由里，则进入New状态
/// * 如果从关联的DeviceInfo获得或者被动收到广播，则进入Ready状态
/// * 如果在路由里配置了忽略，则进入Ignore状态
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Serialize, Deserialize)]
pub enum ChunkState {
    Unknown = 0,
    NotFound = 1, // 不存在
    Pending = 2,  // 准备中
    OnAir = 3,
    Ready = 4,  // 就绪
    Ignore = 5, // 被忽略
}

impl ChunkState {
    pub fn as_u8(&self) -> u8 {
        u8::from(self)
    }
}

// impl Into<u8> for ChunkState {
//     fn into(self) -> u8 {
//         self as u8
//     }
// }

impl TryFrom<u8> for ChunkState {
    type Error = BuckyError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(ChunkState::Unknown),
            1 => Ok(ChunkState::NotFound),
            2 => Ok(ChunkState::Pending),
            3 => Ok(ChunkState::OnAir),
            4 => Ok(ChunkState::Ready),
            5 => Ok(ChunkState::Ignore),
            _ => {
                let msg = format!("unknown chunk-state: {}", value);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidData, msg))
            }
        }
    }
}

impl From<ChunkState> for u8 {
    fn from(value: ChunkState) -> Self {
        let r = &value;
        r.into()
    }
}

impl From<&ChunkState> for u8 {
    fn from(value: &ChunkState) -> Self {
        match value {
            ChunkState::Unknown => 0,
            ChunkState::NotFound => 1,
            ChunkState::Pending => 2,
            ChunkState::OnAir => 3,
            ChunkState::Ready => 4,
            ChunkState::Ignore => 5,
        }
    }
}

// impl From<&ChunkState> for &str {
//     fn from(value: &ChunkState) -> Self {
//         match value {
//             ChunkState::NotFound=>"NotFound",
//             ChunkState::New=>"New",
//             ChunkState::Pending=>"Pending",
//             ChunkState::Ready=>"Ready",
//             ChunkState::Ignore=>"Ignore",
//         }
//     }
// }

#[cfg(test)]
mod test {
    use super::ChunkId;
    use crate::*;

    use std::convert::TryFrom;
    use std::str::FromStr;
    use generic_array::typenum::{marker_traits::Unsigned, U32};
    use rand::RngCore;

    #[test]
    fn chunk() {
        let hash = HashValue::default();
        let chunk_id = ChunkId::new(&hash, 100);
        let chunk_id_str = chunk_id.to_string();
        let chunk_id_str2 = chunk_id.as_object_id().to_string();
        assert_eq!(chunk_id_str, chunk_id_str2);
        println!("chunk_id_str:{}", chunk_id_str);

        let chunk_id_from_str = ChunkId::from_str(&chunk_id_str).unwrap();
        println!("chunk_id_from_str:{:?}", chunk_id_from_str);

        assert_eq!(chunk_id.obj_type_code().unwrap(), ObjectTypeCode::Chunk);

        // 测试chunk_id和object_id的转换
        let obj_id = chunk_id.object_id();
        assert_eq!(obj_id.obj_type_code(), ObjectTypeCode::Chunk);

        {
            let new_chunk_id = ChunkId::try_from(&obj_id).unwrap();
            assert_eq!(new_chunk_id, chunk_id);
        }

        assert_eq!(U32::to_usize(), 32);
    }

    #[test]
    fn chunk2() {
        let mut chunk_data = [0u8;1024*1024];
        let chunk_len = chunk_data.len();
        rand::thread_rng().fill_bytes(&mut chunk_data);
        let chunk_hash = hash_data(&chunk_data);
        println!("{:?}", chunk_hash);
        let chunkid = ChunkId::new(&chunk_hash, chunk_len as u32);
        println!("{:?}", chunkid);
    }
}

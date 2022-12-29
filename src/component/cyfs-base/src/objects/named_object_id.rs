use crate::*;

use std::cmp::Ordering;
use std::convert::TryFrom;
use std::fmt;
use std::fmt::Formatter;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::str::FromStr;

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

        if !T::is_standard_object() {
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

        let id = ObjectId::clone_from_slice(&hash_value).unwrap();

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

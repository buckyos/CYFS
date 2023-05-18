use crate::*;

use base58::ToBase58;
use generic_array::typenum::{marker_traits::Unsigned, U16};
use generic_array::GenericArray;
use std::fmt;

// unique id in const info
#[derive(Clone, Eq, PartialEq)]
pub struct UniqueId(GenericArray<u8, U16>);

impl std::fmt::Debug for UniqueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "UniqueId: {}", self.0.as_slice().to_base58())
    }
}

impl std::fmt::Display for UniqueId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_slice().to_base58())
    }
}

impl From<GenericArray<u8, U16>> for UniqueId {
    fn from(hash: GenericArray<u8, U16>) -> Self {
        Self(hash)
    }
}

impl From<UniqueId> for GenericArray<u8, U16> {
    fn from(hash: UniqueId) -> Self {
        hash.0
    }
}

impl AsRef<GenericArray<u8, U16>> for UniqueId {
    fn as_ref(&self) -> &GenericArray<u8, U16> {
        &self.0
    }
}

impl Default for UniqueId {
    fn default() -> Self {
        Self(GenericArray::default())
    }
}

impl UniqueId {
    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }

    pub fn clone_from_slice(slice: &[u8]) -> Self {
        UniqueId(GenericArray::clone_from_slice(slice))
    }

    pub fn create(slice: &[u8]) -> Self {
        let mut unique_slice = [0u8; 16];
        let mut count = 0;
        for c in slice {
            unique_slice[count] = *c;
            count += 1;
            if count >= 16 {
                break;
            }
        }

        UniqueId::clone_from_slice(&unique_slice)
    }

    // 从源计算hash256，然后取前16bytes做uniqueId
    pub fn create_with_hash(src: &[u8]) -> Self {
        use sha2::Digest;

        let mut sha256 = sha2::Sha256::new();
        sha256.input(src);
        Self::create(&sha256.result())
    }

    pub fn create_with_random() -> Self {
        let mut id = Self::default();
        let (l, h) = id.as_mut_slice().split_at_mut(8);
        l.copy_from_slice(&rand::random::<u64>().to_be_bytes());
        h.copy_from_slice(&rand::random::<u64>().to_be_bytes());
        id
    }
}

impl RawFixedBytes for UniqueId {
    fn raw_bytes() -> Option<usize> {
        Some(U16::to_usize())
    }
}

impl RawEncode for UniqueId {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        Ok(U16::to_usize())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for encode UniqueId, except={}, got={}",
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

impl<'de> RawDecode<'de> for UniqueId {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for decode UniqueId, except={}, got={}",
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

impl ProtobufTransform<&UniqueId> for Vec<u8> {
    fn transform(value: &UniqueId) -> BuckyResult<Self> {
        Ok(Vec::from(value.0.as_slice()))
    }
}

impl ProtobufTransform<Vec<u8>> for UniqueId {
    fn transform(value: Vec<u8>) -> BuckyResult<Self> {
        if value.len() != 32 {
            return Err(BuckyError::new(
                BuckyErrorCode::InvalidParam,
                format!(
                    "try convert from vec<u8> to named unique id failed, invalid len {}",
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

#[cfg(test)]
mod test {
    use crate::*;

    #[test]
    fn test_codec() {
        let id = UniqueId::default();
        let len = id.raw_measure(&None).unwrap();
        assert_eq!(len, 16);
        let buf = id.to_vec().unwrap();
        let (id2, left) = UniqueId::raw_decode(&buf).unwrap();
        assert_eq!(id, id2);
        assert!(left.is_empty());

        let hash = id.raw_hash_value().unwrap();
        println!("hash: {}", hash);
    }
}

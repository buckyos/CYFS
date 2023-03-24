use crate::*;

use aes::{Aes256, BlockCipher};
use block_modes::block_padding::Pkcs7;
use block_modes::{BlockMode, Cbc};
use generic_array::typenum::{marker_traits::Unsigned, U48, U8};
use generic_array::GenericArray;
use sha2::Digest;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::hash::Hash;
use std::str::FromStr;

// aes key used to crypto data
#[derive(Clone, Eq, PartialEq)]
pub struct AesKey(GenericArray<u8, U48>);

impl std::fmt::Debug for AesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}
impl std::fmt::Display for AesKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_base58())
    }
}

impl From<GenericArray<u8, U48>> for AesKey {
    fn from(aes_key: GenericArray<u8, U48>) -> Self {
        Self(aes_key)
    }
}

impl From<AesKey> for GenericArray<u8, U48> {
    fn from(aes_key: AesKey) -> Self {
        aes_key.0
    }
}

impl AsRef<GenericArray<u8, U48>> for AesKey {
    fn as_ref(&self) -> &GenericArray<u8, U48> {
        &self.0
    }
}

impl RawFixedBytes for AesKey {
    fn raw_bytes() -> Option<usize> {
        Some(U48::to_usize())
    }
}

impl RawEncode for AesKey {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(U48::to_usize())
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for encode AesKey, except={}, got={}",
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

impl<'de> RawDecode<'de> for AesKey {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for decode AesKey, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        let mut hash = GenericArray::default();
        unsafe {
            std::ptr::copy(buf.as_ptr(), hash.as_mut_slice().as_mut_ptr(), bytes);
        }
        Ok((Self(hash), &buf[bytes..]))
    }
}

impl From<&[u8; 48]> for AesKey {
    fn from(v: &[u8; 48]) -> Self {
        Self(GenericArray::clone_from_slice(v))
    }
}

impl From<Vec<u8>> for AesKey {
    fn from(v: Vec<u8>) -> Self {
        let ar: [u8; 48] = v.try_into().unwrap_or_else(|v: Vec<u8>| {
            panic!(
                "AesKey expected a Vec of length {} but it was {}",
                48,
                v.len()
            )
        });

        Self(GenericArray::clone_from_slice(&ar))
    }
}

impl Default for AesKey {
    fn default() -> Self {
        Self(GenericArray::default())
    }
}

impl AesKey {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn to_base58(&self) -> String {
        self.0.as_slice().to_base58()
    }

    pub fn from_base58(s: &str) -> BuckyResult<Self> {
        let buf = s.from_base58().map_err(|e| {
            let msg = format!(
                "convert base58 str to AesKey buf failed, str={}, {:?}",
                s, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        if buf.len() != 48 {
            let msg = format!(
                "convert base58 str to AesKey failed, len unmatch: str={}",
                s
            );
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(Self::from(buf))
    }

    pub fn proxy(n: u64) -> AesKey {
        let mut key = [0u8; 48];
        for i in 0..3 {
            key[i * 8..(i + 1) * 8].copy_from_slice(&n.to_be_bytes());
        }
        for i in 3..6 {
            let r = rand::random::<u64>();
            key[i * 8..(i + 1) * 8].copy_from_slice(&r.to_be_bytes());
        }
        AesKey::from(&key)
    }

    pub fn random() -> AesKey {
        let mut key = [0u8; 48];
        for i in 0..6 {
            let r = rand::random::<u64>();
            key[i * 8..(i + 1) * 8].copy_from_slice(&r.to_be_bytes());
        }
        AesKey::from(&key)
    }

    pub fn mix_hash(&self, salt: Option<u64>) -> KeyMixHash {
        let mut sha = sha2::Sha256::new();
        sha.input(self.0.as_slice());
        if let Some(salt) = salt {
            sha.input(&salt.to_le_bytes());
        }

        let hash = sha.result();
        let mut mix_hash =
            GenericArray::from_slice(&hash.as_slice()[..KeyMixHash::raw_bytes().unwrap()]).clone();
        mix_hash[0] = mix_hash[0] & 0x7f;
        KeyMixHash(mix_hash)
    }

    pub fn padded_len(in_len: usize) -> usize {
        let block_size = <Aes256 as BlockCipher>::BlockSize::to_usize();
        block_size * ((in_len / block_size) + 1)
    }

    pub fn encrypt(
        &self,
        in_buf: &[u8],
        out: &mut [u8],
        in_len: usize,
    ) -> Result<usize, BuckyError> {
        out[..in_len].copy_from_slice(&in_buf[..in_len]);
        self.inplace_encrypt(out, in_len)
    }

    pub fn decrypt(
        &self,
        in_buf: &[u8],
        out: &mut [u8],
        in_len: usize,
    ) -> Result<usize, BuckyError> {
        out[..in_len].copy_from_slice(&in_buf[..in_len]);
        self.inplace_decrypt(out, in_len)
    }

    pub fn inplace_encrypt(&self, inout: &mut [u8], in_len: usize) -> Result<usize, BuckyError> {
        // let iv: [u8;16] = [0;16];

        // let buf_len = inout.len();
        // let target_len = (in_len/16 + 1) * 16;
        // if buf_len < target_len {
        //     return Err(BuckyError::from(BuckyErrorCode::OpensslError));
        // }
        // let padding = (target_len - in_len) as u8;
        // for i in in_len..target_len {
        //     inout[i] = padding;
        // }
        let key = self.0.as_slice();
        let cipher = Cbc::<Aes256, Pkcs7>::new_from_slices(&key[0..32], &key[32..]).unwrap();

        match cipher.encrypt(inout, in_len) {
            Ok(buf) => Ok(buf.len()),
            Err(e) => {
                let msg = format!(
                    "AesKey::inplace_encrypt error, inout={}, in_len={}, {}",
                    inout.len(),
                    in_len,
                    e
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg))
            }
        }
    }

    pub fn inplace_decrypt(&self, inout: &mut [u8], in_len: usize) -> Result<usize, BuckyError> {
        // let iv: [u8;16] = [0;16];

        let key = self.0.as_slice();
        let cipher = Cbc::<Aes256, Pkcs7>::new_from_slices(&key[0..32], &key[32..]).unwrap();
        match cipher.decrypt(&mut inout[..in_len]) {
            Ok(buf) => Ok(buf.len()),
            Err(e) => {
                let msg = format!(
                    "AesKey::inplace_decrypt error, inout={}, in_len={}, {}",
                    inout.len(),
                    in_len,
                    e
                );
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg))
            }
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}

impl FromStr for AesKey {
    type Err = BuckyError;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::from_base58(s)
    }
}
// aes key 的mixhash
#[derive(Eq, PartialEq, Hash, Clone, Ord, PartialOrd, Debug)]
pub struct KeyMixHash(GenericArray<u8, U8>);

impl AsRef<GenericArray<u8, U8>> for KeyMixHash {
    fn as_ref(&self) -> &GenericArray<u8, U8> {
        &self.0
    }
}

impl AsMut<GenericArray<u8, U8>> for KeyMixHash {
    fn as_mut(&mut self) -> &mut GenericArray<u8, U8> {
        &mut self.0
    }
}

impl Display for KeyMixHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self.0.as_slice()))
    }
}

impl RawFixedBytes for KeyMixHash {
    fn raw_bytes() -> Option<usize> {
        Some(U8::to_usize())
    }
}

impl RawEncode for KeyMixHash {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(U8::to_usize())
    }
    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for encode KeyMixHash, except={}, got={}",
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

impl<'de> RawDecode<'de> for KeyMixHash {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let bytes = Self::raw_bytes().unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for decode KeyMixHash, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        let mut hash = GenericArray::default();
        unsafe {
            std::ptr::copy(buf.as_ptr(), hash.as_mut_slice().as_mut_ptr(), bytes);
        }
        Ok((Self(hash), &buf[bytes..]))
    }
}

#[cfg(test)]
mod test_aes {
    use super::AesKey;
    use generic_array::typenum::U48;
    use generic_array::GenericArray;

    #[test]
    fn test() {
        let key = b"\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0F\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0A\x0B\x0C\x0D\x0E\x0E";
        let array = GenericArray::<u8, U48>::clone_from_slice(key);
        let aes_key = AesKey(array);
        let mut data: [u8; 128] = [30; 128];
        let d = b"Some Crypto Text11111dsfasdfsdsdSome Crypto Text1111";
        data[..d.len()].copy_from_slice(d);

        assert!(aes_key.inplace_encrypt(data.as_mut(), d.len()).is_ok());
        assert!(aes_key
            .inplace_decrypt(data.as_mut(), (d.len() / 16 + 1) * 16)
            .is_ok());
        assert_eq!(
            String::from_utf8(data[..d.len()].to_vec()),
            String::from_utf8(d.to_vec())
        );
    }
}

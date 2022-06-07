use generic_array::typenum::{marker_traits::Unsigned, U16, U32, U64};
use generic_array::GenericArray;
use std::ptr::slice_from_raw_parts;

use crate::*;

pub const SIGNATURE_REF_INDEX: u8 = 0b_00000000;
pub const SIGNATURE_OBJECT: u8 = 0b_00000001;
pub const SIGNATURE_KEY: u8 = 0b_00000010;

// 1.obj_desc.ref_objs,取值范围为[0, 127]
pub const SIGNATURE_SOURCE_REFINDEX_REF_OBJ_BEGIN: u8 = 0;
pub const SIGNATURE_SOURCE_REFINDEX_REF_OBJ_END: u8 = 127;

/*
2.逻辑ref (从128-255（可以根据需要扩展）
ref[255] = 自己 （适用于有权对象）
ref[254] = owner （使用于有主对象）
ref[253] = author (适用于填写了作者的对象）
ref[252-236] = ood_list[x] (适用于所在Zone的ood对象）
*/
pub const SIGNATURE_SOURCE_REFINDEX_SELF: u8 = 255;
pub const SIGNATURE_SOURCE_REFINDEX_OWNER: u8 = 254;
pub const SIGNATURE_SOURCE_REFINDEX_AUTHOR: u8 = 253;

pub const SIGNATURE_SOURCE_REFINDEX_ZONE_OOD_BEGIN: u8 = 252;
pub const SIGNATURE_SOURCE_REFINDEX_ZONE_OOD_END: u8 = 236;

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum SignatureSource {
    RefIndex(u8),
    Object(ObjectLink),
    Key(PublicKeyValue),
}

impl Default for Signature {
    fn default() -> Self {
        Self {
            sign_source: SignatureSource::RefIndex(0),
            sign_time: bucky_time_now(),
            sign_key_index: 0,
            sign: SignData::Rsa1024(GenericArray::default()),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum SignData {
    Rsa1024(GenericArray<u32, U32>),
    Rsa2048(GenericArray<u32, U64>),
    Ecc(GenericArray<u32, U16>),
}

impl SignData {
    pub fn sign_type(&self) -> &str {
        match self {
            Self::Rsa1024(_) => "rsa1024",
            Self::Rsa2048(_) => "rsa2048",
            Self::Ecc(_) => "ecc",
        }
    }

    pub fn as_slice<'a>(&self) -> &'a [u8] {
        let sign_slice = match self {
            SignData::Rsa1024(sign) => unsafe {
                &*slice_from_raw_parts(
                    sign.as_ptr() as *const u8,
                    std::mem::size_of::<u32>() * U32::to_usize(),
                )
            },
            SignData::Rsa2048(sign) => unsafe {
                &*slice_from_raw_parts(
                    sign.as_ptr() as *const u8,
                    std::mem::size_of::<u32>() * U64::to_usize(),
                )
            },
            SignData::Ecc(sign) => unsafe {
                &*slice_from_raw_parts(
                    sign.as_ptr() as *const u8,
                    std::mem::size_of::<u32>() * U16::to_usize(),
                )
            },
        };
        sign_slice
    }
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct Signature {
    sign_source: SignatureSource,
    sign_key_index: u8,
    sign_time: u64,
    sign: SignData,
}

impl Signature {
    pub fn new(
        sign_source: SignatureSource,
        sign_key_index: u8,
        sign_time: u64,
        sign: SignData,
    ) -> Self {
        Self {
            sign_source: sign_source,
            sign_key_index,
            sign_time: sign_time,
            sign: sign,
        }
    }

    pub fn sign(&self) -> &SignData {
        &self.sign
    }

    pub fn as_slice<'a>(&self) -> &'a [u8] {
        self.sign.as_slice()
    }

    fn sign_source_with_ref_index(&self) -> u8 {
        match self.sign_source {
            SignatureSource::RefIndex(_index) => {
                // sign_key_index[. . . . . . x x] type[. .]
                SIGNATURE_REF_INDEX | (self.sign_key_index << 2)
            }
            SignatureSource::Object(_) => SIGNATURE_OBJECT | (self.sign_key_index << 2),
            SignatureSource::Key(_) => SIGNATURE_KEY,
        }
    }

    pub fn is_ref_index(&self) -> bool {
        match self.sign_source {
            SignatureSource::RefIndex(_) => true,
            _ => false,
        }
    }

    pub fn is_object(&self) -> bool {
        match self.sign_source {
            SignatureSource::Object(_) => true,
            _ => false,
        }
    }

    pub fn is_key(&self) -> bool {
        match self.sign_source {
            SignatureSource::Key(_) => true,
            _ => false,
        }
    }

    pub fn sign_source(&self) -> &SignatureSource {
        &self.sign_source
    }

    pub fn sign_time(&self) -> u64 {
        self.sign_time
    }

    pub fn sign_key_index(&self) -> u8 {
        self.sign_key_index
    }

    pub fn compare_source(&self, other: &Self) -> bool {
        self.sign_source == other.sign_source && self.sign_key_index == other.sign_key_index
    }
}

impl RawEncode for Signature {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        // sign_source_with_ref_index
        let mut size = u8::raw_bytes().unwrap();

        // signatory: Option<SignatureSource>
        size = size
            + match &self.sign_source {
                SignatureSource::RefIndex(_) => u8::raw_bytes().unwrap(),
                SignatureSource::Object(obj) => obj.raw_measure(purpose)?,
                SignatureSource::Key(key) => key.raw_measure(purpose)?,
            };

        // sign_time: u64
        size = size + u64::raw_bytes().unwrap();

        // sign_data: Vec<u8>
        size = size
            + u8::raw_bytes().unwrap()
            + std::mem::size_of::<u32>()
                * match self.sign {
                    SignData::Rsa1024(_) => U32::to_usize(),
                    SignData::Rsa2048(_) => U64::to_usize(),
                    SignData::Ecc(_) => U16::to_usize(),
                };

        Ok(size)
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let bytes = self.raw_measure(purpose).unwrap();
        if buf.len() < bytes {
            let msg = format!(
                "not enough buffer for encode Signature buf, except={}, got={}",
                bytes,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }

        // sign_source_with_ref_index
        let buf = self.sign_source_with_ref_index().raw_encode(buf, purpose)?;

        // signatory: Option<SignatureSource>
        let buf = match &self.sign_source {
            SignatureSource::RefIndex(t) => {
                let buf = t.raw_encode(buf, purpose)?;
                buf
            }
            SignatureSource::Object(t) => {
                let buf = t.raw_encode(buf, purpose)?;
                buf
            }
            SignatureSource::Key(t) => {
                let buf = t.raw_encode(buf, purpose)?;
                buf
            }
        };

        // sign_time
        let buf = self.sign_time.raw_encode(buf, purpose)?;

        // sign_data: Vec<u8>
        let buf = match self.sign {
            SignData::Rsa1024(sign) => {
                let buf = KEY_TYPE_RSA.raw_encode(buf, purpose)?;
                let bytes = std::mem::size_of::<u32>() * U32::to_usize();
                unsafe {
                    std::ptr::copy(
                        sign.as_slice().as_ptr() as *const u8,
                        buf.as_mut_ptr(),
                        bytes,
                    );
                }
                &mut buf[bytes..]
            }
            SignData::Rsa2048(sign) => {
                let buf = KEY_TYPE_RSA2048.raw_encode(buf, purpose)?;
                let bytes = std::mem::size_of::<u32>() * U64::to_usize();
                unsafe {
                    std::ptr::copy(
                        sign.as_slice().as_ptr() as *const u8,
                        buf.as_mut_ptr(),
                        bytes,
                    );
                }
                &mut buf[bytes..]
            }
            SignData::Ecc(sign) => {
                let buf = KEY_TYPE_SECP256K1.raw_encode(buf, purpose)?;
                let bytes = std::mem::size_of::<u32>() * U16::to_usize();
                unsafe {
                    std::ptr::copy(
                        sign.as_slice().as_ptr() as *const u8,
                        buf.as_mut_ptr(),
                        bytes,
                    );
                }
                &mut buf[bytes..]
            }
        };

        Ok(buf)
    }
}

impl<'de> RawDecode<'de> for Signature {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        // [. . . . . . ]  [. .]
        // ref_index     | real_type_code
        let (sign_source_with_ref_index, buf) = u8::raw_decode(buf)?;

        let sign_source_code = sign_source_with_ref_index << 6 >> 6;
        let sign_key_index = sign_source_with_ref_index >> 2;

        let (sign_source, buf) = match sign_source_code {
            SIGNATURE_REF_INDEX => {
                let (ref_index, buf) = u8::raw_decode(buf)?;
                Ok((SignatureSource::RefIndex(ref_index), buf))
            }
            SIGNATURE_OBJECT => {
                let (obj_link, buf) = ObjectLink::raw_decode(buf)?;
                Ok((SignatureSource::Object(obj_link), buf))
            }
            SIGNATURE_KEY => {
                let (key, buf) = PublicKeyValue::raw_decode(buf)?;
                Ok((SignatureSource::Key(key), buf))
            }
            _ => Err(BuckyError::from("invalid signature type")),
        }?;

        let (sign_time, buf) = u64::raw_decode(buf)?;

        let (key_type, buf) = u8::raw_decode(buf)?;

        let (sign, buf) = match key_type {
            KEY_TYPE_RSA => {
                let bytes = std::mem::size_of::<u32>() * U32::to_usize();
                if buf.len() < bytes {
                    return Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "not enough buffer for rsa1024 signature",
                    ));
                }

                let mut sign = GenericArray::default();
                unsafe {
                    std::ptr::copy(
                        buf.as_ptr(),
                        sign.as_mut_slice().as_mut_ptr() as *mut u8,
                        bytes,
                    );
                }

                (SignData::Rsa1024(sign), &buf[bytes..])
            }
            KEY_TYPE_RSA2048 => {
                let bytes = std::mem::size_of::<u32>() * U64::to_usize();
                if buf.len() < bytes {
                    return Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "not enough buffer for rsa2048 signature",
                    ));
                }

                let mut sign = GenericArray::default();
                unsafe {
                    std::ptr::copy(
                        buf.as_ptr(),
                        sign.as_mut_slice().as_mut_ptr() as *mut u8,
                        bytes,
                    );
                }

                (SignData::Rsa2048(sign), &buf[bytes..])
            }
            KEY_TYPE_SECP256K1 => {
                let bytes = std::mem::size_of::<u32>() * U16::to_usize();
                if buf.len() < bytes {
                    return Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "not enough buffer for secp256k1 signature",
                    ));
                }

                let mut sign = GenericArray::default();
                unsafe {
                    std::ptr::copy(
                        buf.as_ptr(),
                        sign.as_mut_slice().as_mut_ptr() as *mut u8,
                        bytes,
                    );
                }

                (SignData::Ecc(sign), &buf[bytes..])
            }
            _ => {
                return Err(BuckyError::new(
                    BuckyErrorCode::NotMatch,
                    format!("Invalid Signature KeyType:{}", key_type),
                ));
            }
        };

        Ok((
            Self {
                sign_source: sign_source,
                sign_key_index,
                sign_time,
                sign: sign,
            },
            buf,
        ))
    }
}

#[cfg(test)]
mod test {
    use crate::{RawConvertTo, RawFrom, Signature};

    #[test]
    fn signature() {
        let sig1 = Signature::default();
        let buf = sig1.to_vec().unwrap();
        let sig2 = Signature::clone_from_slice(&buf).unwrap();
        assert_eq!(sig1, sig2)
    }
}

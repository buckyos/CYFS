use crate::*;

use rand::thread_rng;
use rsa::{PublicKey as RSAPublicKeyTrait, PublicKeyParts};

use std::convert::From;

// RSA
const RAW_PUBLIC_KEY_RSA_1024_CODE: u8 = 0_u8;
const RAW_PUBLIC_KEY_RSA_1024_LENGTH: usize = 162;

const RAW_PUBLIC_KEY_RSA_2048_CODE: u8 = 1_u8;
const RAW_PUBLIC_KEY_RSA_2048_LENGTH: usize = 294;

const RAW_PUBLIC_KEY_RSA_3072_CODE: u8 = 2_u8;
const RAW_PUBLIC_KEY_RSA_3072_LENGTH: usize = 422;

// SECP256K1
const RAW_PUBLIC_KEY_SECP256K1_CODE: u8 = 10_u8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PublicKey {
    Rsa(rsa::RSAPublicKey),
    Secp256k1(::secp256k1::PublicKey),
    Invalid,
}

impl Default for PublicKey {
    fn default() -> Self {
        PublicKey::Invalid
    }
}

impl PublicKey {
    pub fn key_type_str(&self) -> &str {
        match self {
            Self::Rsa(_) => PrivateKeyType::Rsa.as_str(),
            Self::Secp256k1(_) => PrivateKeyType::Secp256k1.as_str(),
            Self::Invalid => "invalid",
        }
    }

    pub fn key_size(&self) -> usize {
        match self {
            Self::Rsa(pk) => pk.size() as usize,
            Self::Secp256k1(_) => {
                // 采用压缩格式存储 33个字节
                ::secp256k1::util::COMPRESSED_PUBLIC_KEY_SIZE
            }
            Self::Invalid => panic!("Should not come here"),
        }
    }

    pub fn encrypt(&self, data: &[u8], output: &mut [u8]) -> BuckyResult<usize> {
        let encrypted_buf = self.encrypt_data(data)?;
        if output.len() < encrypted_buf.len() {
            let msg = format!(
                "not enough buffer for public key encrypt buf: {} < {}",
                output.len(),
                encrypted_buf.len()
            );
            Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg))
        } else {
            output[..encrypted_buf.len()].copy_from_slice(encrypted_buf.as_slice());
            Ok(encrypted_buf.len())
        }
    }

    pub fn encrypt_data(&self, data: &[u8]) -> BuckyResult<Vec<u8>> {
        match self {
            Self::Rsa(public_key) => {
                let mut rng = thread_rng();
                let encrypted_buf =
                    match public_key.encrypt(&mut rng, rsa::PaddingScheme::PKCS1v15Encrypt, data) {
                        Ok(v) => v,
                        Err(e) => match e {
                            rsa::errors::Error::MessageTooLong => {
                                let msg = format!(
                                    "encrypt data is too long! data len={}, max len={}",
                                    data.len(),
                                    public_key.size() - 11
                                );
                                error!("{}", msg);
                                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
                            }
                            _ => return Err(BuckyError::from(e)),
                        },
                    };

                Ok(encrypted_buf)
            }
            Self::Secp256k1(_) => {
                // 目前secp256k1的非对称加解密只支持交换aes_key时候使用
                let msg = format!("direct encyrpt with private key of secp256 not support!");
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
            PublicKey::Invalid => panic!("Should not come here"),
        }
    }

    pub fn gen_aeskey_and_encrypt(&self) -> BuckyResult<(AesKey, Vec<u8>)> {
        match self {
            Self::Rsa(pk) => {
                // 先产生一个临时的aes_key
                let key = AesKey::random();

                // 使用publicKey对aes_key加密
                let mut output = Vec::with_capacity(pk.size());
                unsafe {
                    output.set_len(pk.size());
                };
                self.encrypt(key.as_slice(), &mut output)?;

                Ok((key, output))
            }
            Self::Secp256k1(public_key) => {
                let (ephemeral_sk, ephemeral_pk) = cyfs_ecies::utils::generate_keypair();

                let aes_key = cyfs_ecies::utils::encapsulate(&ephemeral_sk, &public_key);
                let pk_buf = ephemeral_pk.serialize_compressed();

                let key = AesKey::from(&aes_key);
                Ok((key, pk_buf.to_vec()))
            }
            Self::Invalid => panic!("Should not come here"),
        }
    }

    pub fn verify(&self, data: &[u8], sign: &Signature) -> bool {
        let create_time = sign.sign_time();
        let mut data_new = data.to_vec();
        data_new.resize(data.len() + create_time.raw_measure(&None).unwrap(), 0);
        create_time
            .raw_encode(&mut data_new.as_mut_slice()[data.len()..], &None)
            .unwrap();

        match self {
            Self::Rsa(public_key) => {
                let hash = hash_data(&data_new);
                public_key
                    .verify(
                        rsa::PaddingScheme::new_pkcs1v15_sign(Some(rsa::Hash::SHA2_256)),
                        hash.as_slice(),
                        sign.as_slice(),
                    )
                    .is_ok()
            }
            Self::Secp256k1(public_key) => {
                // 生成消息摘要
                let hash = hash_data(&data_new);
                assert_eq!(HashValue::len(), ::secp256k1::util::MESSAGE_SIZE);
                let ctx = ::secp256k1::Message::parse(hash.as_slice().try_into().unwrap());

                // 解析签名段
                let sign = match ::secp256k1::Signature::parse_slice(sign.as_slice()) {
                    Ok(sign) => sign,
                    Err(e) => {
                        error!("parse secp256k1 signature error: {}", e);
                        return false;
                    }
                };

                // 使用公钥进行校验
                secp256k1::verify(&ctx, &sign, &public_key)
            }
            Self::Invalid => panic!("Should not come here"),
        }
    }
}

impl RawFixedBytes for PublicKey {
    fn raw_min_bytes() -> Option<usize> {
        Some(1 + RAW_PUBLIC_KEY_RSA_1024_LENGTH)
    }

    fn raw_max_bytes() -> Option<usize> {
        Some(1 + RAW_PUBLIC_KEY_RSA_3072_LENGTH)
    }
}

impl RawEncode for PublicKey {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        match self {
            Self::Rsa(ref _pk) => {
                match _pk.size() {
                    // 1024 bits, 128 bytes
                    128 => Ok(RAW_PUBLIC_KEY_RSA_1024_LENGTH + 1),
                    256 => Ok(RAW_PUBLIC_KEY_RSA_2048_LENGTH + 1),
                    384 => Ok(RAW_PUBLIC_KEY_RSA_3072_LENGTH + 1),
                    _ => Err(BuckyError::new(
                        BuckyErrorCode::InvalidParam,
                        "invalid rsa public key",
                    )),
                }
            }
            Self::Secp256k1(_) => Ok(::secp256k1::util::COMPRESSED_PUBLIC_KEY_SIZE + 1),
            Self::Invalid => {
                let msg = format!("invalid publicKey!");
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        _purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        match self {
            Self::Rsa(ref pk) => {
                let (code, len) = {
                    match pk.size() {
                        128 => Ok((
                            RAW_PUBLIC_KEY_RSA_1024_CODE,
                            RAW_PUBLIC_KEY_RSA_1024_LENGTH + 1,
                        )),
                        256 => Ok((
                            RAW_PUBLIC_KEY_RSA_2048_CODE,
                            RAW_PUBLIC_KEY_RSA_2048_LENGTH + 1,
                        )),
                        384 => Ok((
                            RAW_PUBLIC_KEY_RSA_3072_CODE,
                            RAW_PUBLIC_KEY_RSA_3072_LENGTH + 1,
                        )),
                        _ => Err(BuckyError::new(
                            BuckyErrorCode::InvalidParam,
                            "invalid rsa public key",
                        )),
                    }
                }?;
                if buf.len() < len {
                    let msg = format!(
                        "not enough buffer for encode privateKey, except={}, got={}",
                        len,
                        buf.len()
                    );
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
                }

                let spki_der = rsa_export::pkcs1::public_key(pk)?;
                assert!(spki_der.len() <= len - 1);
                buf[0] = code;
                buf[1..spki_der.len() + 1].copy_from_slice(&spki_der.as_slice());
                buf[spki_der.len() + 1..]
                    .iter_mut()
                    .for_each(|padding| *padding = 0);

                Ok(&mut buf[len..])
            }
            Self::Secp256k1(public_key) => {
                buf[0] = RAW_PUBLIC_KEY_SECP256K1_CODE;
                let key_buf = public_key.serialize_compressed();
                let total_len = ::secp256k1::util::COMPRESSED_PUBLIC_KEY_SIZE + 1;
                buf[1..total_len].copy_from_slice(&key_buf);

                Ok(&mut buf[total_len..])
            }
            Self::Invalid => panic!("should not reach here"),
        }
    }
}

impl<'de> RawDecode<'de> for PublicKey {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        if buf.len() < 1 {
            let msg = format!(
                "not enough buffer for decode PublicKey, min bytes={}, got={}",
                1,
                buf.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
        }
        let code = buf[0];
        match code {
            RAW_PUBLIC_KEY_RSA_1024_CODE
            | RAW_PUBLIC_KEY_RSA_2048_CODE
            | RAW_PUBLIC_KEY_RSA_3072_CODE => {
                let len = {
                    match code {
                        RAW_PUBLIC_KEY_RSA_1024_CODE => RAW_PUBLIC_KEY_RSA_1024_LENGTH + 1,
                        RAW_PUBLIC_KEY_RSA_2048_CODE => RAW_PUBLIC_KEY_RSA_2048_LENGTH + 1,
                        RAW_PUBLIC_KEY_RSA_3072_CODE => RAW_PUBLIC_KEY_RSA_3072_LENGTH + 1,
                        _ => unreachable!(),
                    }
                };
                if buf.len() < len {
                    let msg = format!(
                        "not enough buffer for decode rsa PublicKey, except={}, got={}",
                        len,
                        buf.len()
                    );
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
                }
                let pk = rsa::RSAPublicKey::from_pkcs1(&buf[1..len])?;
                Ok((PublicKey::Rsa(pk), &buf[len..]))
            }
            RAW_PUBLIC_KEY_SECP256K1_CODE => {
                let len = ::secp256k1::util::COMPRESSED_PUBLIC_KEY_SIZE + 1;
                if buf.len() < len {
                    let msg = format!(
                        "not enough buffer for decode secp256k1 PublicKey, except={}, got={}",
                        len,
                        buf.len()
                    );
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::OutOfLimit, msg));
                }

                match ::secp256k1::PublicKey::parse_compressed((&buf[1..len]).try_into().unwrap()) {
                    Ok(public_key) => Ok((PublicKey::Secp256k1(public_key), &buf[len..])),
                    Err(e) => {
                        let msg = format!("parse secp256k1 public key error: {}", e);
                        error!("{}", msg);

                        Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
                    }
                }
            }
            v @ _ => Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                &format!("invalid public key type code {}", v),
            )),
        }
    }
}

// threshold, public_key list
pub type MNPublicKey = (u8, Vec<PublicKey>);

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum PublicKeyValue {
    Single(PublicKey),
    MN(MNPublicKey),
}

pub enum PublicKeyRef<'a> {
    Single(&'a PublicKey),
    MN(&'a MNPublicKey),
}

impl PublicKeyValue {
    pub fn as_ref(&self) -> PublicKeyRef {
        match self {
            Self::Single(v) => PublicKeyRef::Single(v),
            Self::MN(v) => PublicKeyRef::MN(v),
        }
    }
}

impl<'a> From<&'a PublicKey> for PublicKeyRef<'a> {
    fn from(key: &'a PublicKey) -> Self {
        PublicKeyRef::Single(key)
    }
}

impl<'a> From<&'a MNPublicKey> for PublicKeyRef<'a> {
    fn from(key: &'a MNPublicKey) -> Self {
        PublicKeyRef::MN(key)
    }
}

impl RawEncode for PublicKeyValue {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        match self {
            PublicKeyValue::Single(key) => Ok(u8::raw_bytes().unwrap() + key.raw_measure(purpose)?),
            PublicKeyValue::MN(key) => Ok(u8::raw_bytes().unwrap() + key.raw_measure(purpose)?),
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for publick_key_value",
            ));
        }

        match self {
            PublicKeyValue::Single(key) => {
                let buf = 0u8.raw_encode(buf, purpose)?;
                let buf = key.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            PublicKeyValue::MN(key) => {
                let buf = 1u8.raw_encode(buf, purpose)?;
                let buf = key.raw_encode(buf, purpose)?;
                Ok(buf)
            }
        }
    }
}

impl<'de> RawDecode<'de> for PublicKeyValue {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        let (key_type, buf) = u8::raw_decode(buf)?;

        match key_type {
            0u8 => {
                let (key, buf) = PublicKey::raw_decode(buf)?;
                Ok((PublicKeyValue::Single(key), buf))
            }
            1u8 => {
                let (key, buf) = MNPublicKey::raw_decode(buf)?;
                Ok((PublicKeyValue::MN(key), buf))
            }
            _ => Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                "public key type invalid",
            )),
        }
    }
}

impl<'r> RawEncode for PublicKeyRef<'r> {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        match self {
            PublicKeyRef::Single(key) => Ok(u8::raw_bytes().unwrap() + key.raw_measure(purpose)?),
            PublicKeyRef::MN(key) => Ok(u8::raw_bytes().unwrap() + key.raw_measure(purpose)?),
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> Result<&'a mut [u8], BuckyError> {
        let size = self.raw_measure(purpose).unwrap();
        if buf.len() < size {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "[raw_encode] not enough buffer for public_key",
            ));
        }

        match *self {
            PublicKeyRef::Single(key) => {
                let buf = 0u8.raw_encode(buf, purpose)?;
                let buf = key.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            PublicKeyRef::MN(key) => {
                let buf = 1u8.raw_encode(buf, purpose)?;
                let buf = key.raw_encode(buf, purpose)?;
                Ok(buf)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{PrivateKey, PublicKey, RawConvertTo, RawDecode};

    #[test]
    fn public_key() {
        let sk1 = PrivateKey::generate_rsa(1024).unwrap();
        let pk1_buf = sk1.public().to_vec().unwrap();
        println!("encrypt max len: {}", sk1.public().key_size() - 11);
        let (pk2, buf) = PublicKey::raw_decode(&pk1_buf).unwrap();
        assert!(buf.len() == 0);
        assert_eq!(sk1.public(), pk2);

        let sk1 = PrivateKey::generate_rsa(2048).unwrap();
        let pk1_buf = sk1.public().to_vec().unwrap();
        println!("encrypt max len: {}", sk1.public().key_size() - 11);
        let (pk2, buf) = PublicKey::raw_decode(&pk1_buf).unwrap();
        assert!(buf.len() == 0);
        assert_eq!(sk1.public(), pk2);

        let sk1 = PrivateKey::generate_secp256k1().unwrap();
        let pk1_buf = sk1.to_vec().unwrap();
        let (pk2, buf) = PrivateKey::raw_decode(&pk1_buf).unwrap();
        assert!(buf.len() == 0);
        assert_eq!(sk1, pk2);

        let pk1_buf = sk1.public().to_vec().unwrap();
        let (pk2, buf) = PublicKey::raw_decode(&pk1_buf).unwrap();
        assert!(buf.len() == 0);

        assert_eq!(sk1.public(), pk2);
    }
}

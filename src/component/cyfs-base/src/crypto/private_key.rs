use crate::*;

use generic_array::GenericArray;
use libc::memcpy;
use rand::{thread_rng, Rng};
use rsa::PublicKeyParts;
use std::{os::raw::c_void, str::FromStr};

// 密钥类型的编码
pub(crate) const KEY_TYPE_RSA: u8 = 0u8;
pub(crate) const KEY_TYPE_RSA2048: u8 = 1u8;
pub(crate) const KEY_TYPE_RSA3072: u8 = 2u8;
pub(crate) const KEY_TYPE_SECP256K1: u8 = 5u8;

// rsa key size in bits
pub(crate) const RSA_KEY_BITS: usize = 1024;
pub(crate) const RSA2048_KEY_BITS: usize = 2048;
pub(crate) const RSA3072_KEY_BITS: usize = 3072;

// rsa key size in bytes
pub(crate) const RSA_KEY_BYTES: usize = 128;
pub(crate) const RSA2048_KEY_BYTES: usize = 256;
pub(crate) const RSA3072_KEY_BYTES: usize = 384;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PrivateKeyType {
    Rsa,
    Secp256k1,
}

impl PrivateKeyType {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Rsa => "rsa",
            Self::Secp256k1 => "secp256k1",
        }
    }
}

impl Default for PrivateKeyType {
    fn default() -> Self {
        Self::Rsa
    }
}

impl std::fmt::Display for PrivateKeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for PrivateKeyType {
    type Err = BuckyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "rsa" => Self::Rsa,
            "secp256k1" => Self::Secp256k1,
             _ => {
                let msg = format!("unknown PrivateKey type: {}", s);
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg))
             }
        })
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum PrivateKey {
    Rsa(rsa::RSAPrivateKey),
    Secp256k1(::secp256k1::SecretKey),
}

// 避免私钥被日志打印出来
impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[Protected PrivateKey]")
    }
}
impl std::fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "[Protected PrivateKey]")
    }
}

pub const CYFS_PRIVTAE_KEY_DEFAULT_RSA_BITS: usize = 1024;

impl PrivateKey {
    pub fn key_type(&self) -> PrivateKeyType {
        match *self {
            Self::Rsa(_) => PrivateKeyType::Rsa,
            Self::Secp256k1(_) => PrivateKeyType::Secp256k1,
        }
    }

    fn check_bits(bits: usize) -> BuckyResult<()> {
        match bits {
            RSA_KEY_BITS | RSA2048_KEY_BITS | RSA3072_KEY_BITS=> {
                Ok(())
            }
            _ => {
                let msg = format!("unsupport rsa key bits: {}", bits);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::UnSupport, msg))
            }
        }
    }
    // 生成rsa密钥的相关接口
    pub fn generate_rsa(bits: usize) -> Result<Self, BuckyError> {
        Self::check_bits(bits)?;

        let mut rng = thread_rng();
        Self::generate_rsa_by_rng(&mut rng, bits)
    }

    pub fn generate_rsa_by_rng<R: Rng>(rng: &mut R, bits: usize) -> Result<Self, BuckyError> {
        Self::check_bits(bits)?;

        match rsa::RSAPrivateKey::new(rng, bits) {
            Ok(rsa) => Ok(Self::Rsa(rsa)),
            Err(e) => Err(BuckyError::from(e)),
        }
    }

    // 生成secp256k1密钥的相关接口
    pub fn generate_secp256k1() -> Result<Self, BuckyError> {
        let mut rng = thread_rng();
        Self::generate_secp256k1_by_rng(&mut rng)
    }

    pub fn generate_secp256k1_by_rng<R: Rng>(rng: &mut R) -> Result<Self, BuckyError> {
        let key = ::secp256k1::SecretKey::random(rng);
        Ok(Self::Secp256k1(key))
    }

    pub fn generate_by_rng<R: Rng>(rng: &mut R, bits: Option<usize>, pt: PrivateKeyType) -> BuckyResult<Self> {
        match pt {
            PrivateKeyType::Rsa => Self::generate_rsa_by_rng(rng, bits.unwrap_or(CYFS_PRIVTAE_KEY_DEFAULT_RSA_BITS)),
            PrivateKeyType::Secp256k1 => Self::generate_secp256k1_by_rng(rng)
        }
    }

    pub fn public(&self) -> PublicKey {
        match self {
            Self::Rsa(private_key) => PublicKey::Rsa(private_key.to_public_key()),
            Self::Secp256k1(private_key) => {
                PublicKey::Secp256k1(::secp256k1::PublicKey::from_secret_key(private_key))
            }
        }
    }

    pub fn sign(&self, data: &[u8], sign_source: SignatureSource) -> BuckyResult<Signature> {
        let create_time = bucky_time_now();

        // 签名必须也包含签名的时刻，这个时刻是敏感的不可修改
        let mut data_new = data.to_vec();
        data_new.resize(data.len() + create_time.raw_measure(&None).unwrap(), 0);
        create_time
            .raw_encode(&mut data_new.as_mut_slice()[data.len()..], &None)?;

        let sign = match self {
            Self::Rsa(private_key) => {
                let hash = hash_data(&data_new);
                let sign = private_key
                    .sign(
                        rsa::PaddingScheme::new_pkcs1v15_sign(Some(rsa::Hash::SHA2_256)),
                        &hash.as_slice(),
                    )?;
                    
                assert_eq!(sign.len(), private_key.size());
                let sign_data = match private_key.size() {
                    RSA_KEY_BYTES => {
                        let mut sign_array: [u32; 32] = [0; 32];
                        unsafe {
                            memcpy(
                                sign_array.as_mut_ptr() as *mut c_void,
                                sign.as_ptr() as *const c_void,
                                sign.len(),
                            )
                        };
                        SignData::Rsa1024(GenericArray::from(sign_array))
                    }
                    RSA2048_KEY_BYTES => {
                        let mut sign_array: [u32; 64] = [0; 64];
                        unsafe {
                            memcpy(
                                sign_array.as_mut_ptr() as *mut c_void,
                                sign.as_ptr() as *const c_void,
                                sign.len(),
                            )
                        };
                        SignData::Rsa2048(*GenericArray::from_slice(&sign_array))
                    }
                    RSA3072_KEY_BYTES => {
                        let mut sign_array: [u32; 96] = [0; 96];
                        unsafe {
                            memcpy(
                                sign_array.as_mut_ptr() as *mut c_void,
                                sign.as_ptr() as *const c_void,
                                sign.len(),
                            )
                        };
                        SignData::Rsa3072(*GenericArray::from_slice(&sign_array))
                    }

                    len @ _ =>  {
                        let msg = format!("unsupport rsa key length! {}", len);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                    }
                };

                Signature::new(sign_source, 0, create_time, sign_data)
            }

            Self::Secp256k1(private_key) => {
                let hash = hash_data(&data_new);
                assert_eq!(HashValue::len(), ::secp256k1::util::MESSAGE_SIZE);
                let ctx = ::secp256k1::Message::parse(hash.as_slice().try_into().unwrap());

                let (signature, _) = ::secp256k1::sign(&ctx, &private_key);
                let sign_buf = signature.serialize();

                let mut sign_array: [u32; 16] = [0; 16];
                unsafe {
                    memcpy(
                        sign_array.as_mut_ptr() as *mut c_void,
                        sign_buf.as_ptr() as *const c_void,
                        sign_buf.len(),
                    )
                };
                let sign_data = SignData::Ecc(GenericArray::from(sign_array));
                Signature::new(sign_source, 0, create_time, sign_data)
            }
        };

        Ok(sign)
    }

    pub fn decrypt(&self, input: &[u8], output: &mut [u8]) -> BuckyResult<usize> {
        let buf = self.decrypt_data(input)?;
        if output.len() < buf.len() {
            let msg = format!(
                "rsa decrypt error, except={}, got={}",
                buf.len(),
                output.len()
            );
            error!("{}", msg);

            Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
        } else {
            output[..buf.len()].copy_from_slice(buf.as_slice());
            Ok(buf.len())
        }
    }

    pub fn decrypt_data(&self, input: &[u8]) -> BuckyResult<Vec<u8>> {
        match self {
            Self::Rsa(private_key) => {
                let buf = private_key
                    .decrypt(rsa::PaddingScheme::PKCS1v15Encrypt, input)
                    .map_err(|e| BuckyError::from(e))?;
                Ok(buf)
            }

            Self::Secp256k1(_) => {
                // 目前secp256k1的非对称加解密只支持交换aes_key时候使用
                let msg = format!("direct decyrpt with private key of secp256 not support!");
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    pub fn decrypt_aeskey<'d>(&self, input: &'d [u8], output: &mut [u8]) -> BuckyResult<(&'d [u8], usize)> {
        let (input, data) = self.decrypt_aeskey_data(input)?;
        if output.len() < data.len() {
            let msg = format!(
                "not enough buffer for decrypt aeskey result, except={}, got={}",
                data.len(),
                output.len()
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        output.copy_from_slice(&data);
        Ok((input, data.len()))
    }

    pub fn decrypt_aeskey_data<'d>(&self, input: &'d [u8]) -> BuckyResult<(&'d [u8], Vec<u8>)> {
        match self {
            Self::Rsa(_) => {
                let key_size = self.public().key_size();
                if input.len() < key_size {
                    let msg = format!(
                        "not enough buffer for RSA private key, except={}, got={}",
                        key_size,
                        input.len()
                    );
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                let buf = self.decrypt_data(&input[..key_size])?;

                Ok((&input[key_size..], buf))
            },

            Self::Secp256k1(private_key) => {
                if input.len() < ::secp256k1::util::COMPRESSED_PUBLIC_KEY_SIZE {
                    let msg = format!(
                        "not enough buffer for secp256k1 private key, except={}, got={}",
                        ::secp256k1::util::COMPRESSED_PUBLIC_KEY_SIZE,
                        input.len()
                    );
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                let ephemeral_pk = ::secp256k1::PublicKey::parse_slice(
                    &input[..::secp256k1::util::COMPRESSED_PUBLIC_KEY_SIZE],
                    Some(::secp256k1::PublicKeyFormat::Compressed),
                )
                .map_err(|e| {
                    let msg = format!("parse secp256k1 public key error: {}", e);
                    error!("{}", msg);

                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;
                let aes_key = ::cyfs_ecies::utils::decapsulate(&ephemeral_pk, &private_key);
                
                Ok((&input[::secp256k1::util::COMPRESSED_PUBLIC_KEY_SIZE..], aes_key.into()))
            }
        }
    }
}

impl RawEncode for PrivateKey {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> Result<usize, BuckyError> {
        // 这里直接输出正确长度先，然后看如何优化
        match self {
            Self::Rsa(pk) => {
                let spki_der = rsa_export::pkcs1::private_key(pk)?;
                Ok(spki_der.len() + 3)
            }
            Self::Secp256k1(_) => Ok(::secp256k1::util::SECRET_KEY_SIZE + 1),
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
                "[raw_encode] not enough buffer for privake key for private_key",
            ));
        }

        match self {
            Self::Rsa(pk) => {
                let spki_der = rsa_export::pkcs1::private_key(pk)?;
                let mut buf = KEY_TYPE_RSA.raw_encode(buf, purpose)?;
                buf = (spki_der.len() as u16).raw_encode(buf, purpose)?;
                buf[..spki_der.len()].copy_from_slice(&spki_der.as_slice());
                Ok(&mut buf[spki_der.len()..])
            }
            Self::Secp256k1(pk) => {
                let buf = KEY_TYPE_SECP256K1.raw_encode(buf, purpose)?;

                // 由于长度固定，所以我们这里不需要额外存储一个长度信息了
                let key_buf = pk.serialize();
                buf[..::secp256k1::util::SECRET_KEY_SIZE].copy_from_slice(&key_buf);
                Ok(&mut buf[::secp256k1::util::SECRET_KEY_SIZE..])
            }
        }
    }
}

impl<'de> RawDecode<'de> for PrivateKey {
    fn raw_decode(buf: &'de [u8]) -> Result<(Self, &'de [u8]), BuckyError> {
        if buf.len() < 1 {
            return Err(BuckyError::new(
                BuckyErrorCode::OutOfLimit,
                "not enough buffer for PrivateKey",
            ));
        }
        let (type_code, buf) = u8::raw_decode(buf)?;
        match type_code {
            KEY_TYPE_RSA => {
                let (len, buf) = u16::raw_decode(buf)?;
                if buf.len() < len as usize {
                    return Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "not enough buffer for rsa privateKey",
                    ));
                }
                let der = &buf[..len as usize];
                let private_key = rsa::RSAPrivateKey::from_pkcs1(der)?;
                Ok((PrivateKey::Rsa(private_key), &buf[len as usize..]))
            }
            KEY_TYPE_SECP256K1 => {
                if buf.len() < ::secp256k1::util::SECRET_KEY_SIZE {
                    return Err(BuckyError::new(
                        BuckyErrorCode::OutOfLimit,
                        "not enough buffer for secp256k1 privateKey",
                    ));
                }

                match ::secp256k1::SecretKey::parse_slice(
                    &buf[..::secp256k1::util::SECRET_KEY_SIZE],
                ) {
                    Ok(private_key) => Ok((
                        PrivateKey::Secp256k1(private_key),
                        &buf[::secp256k1::util::SECRET_KEY_SIZE..],
                    )),
                    Err(e) => {
                        let msg = format!("parse secp256k1 private key error: {}", e);
                        error!("{}", e);

                        Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
                    }
                }
            }
            _ => Err(BuckyError::new(
                BuckyErrorCode::InvalidData,
                &format!("invalid private key type code {}", buf[0]),
            )),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{PrivateKey, RawConvertTo, RawDecode, SignatureSource};

    #[test]
    fn private_key() {
        rsa_private_key_sign(1024);
        rsa_private_key_sign(2048);
        rsa_private_key_sign(3072);
    }

    fn rsa_private_key_sign(bits: usize) {
        let msg = b"112233445566778899";
        let pk1 = PrivateKey::generate_rsa(bits).unwrap();
        let sign = pk1.sign(msg, SignatureSource::RefIndex(0)).unwrap();
        assert!(pk1.public().verify(msg, &sign));

        let pk1_buf = pk1.to_vec().unwrap();
        let (pk2, buf) = PrivateKey::raw_decode(&pk1_buf).unwrap();
        assert!(buf.len() == 0);

        assert!(pk2.public().verify(msg, &sign));
    }

    #[test]
    fn crypto() {
        rsa_private_key_crypto(1024);
        rsa_private_key_crypto(2048);
        rsa_private_key_crypto(3072);

        let pk1 = PrivateKey::generate_secp256k1().unwrap();
        let (aes_key, mut data) = pk1.public().gen_aeskey_and_encrypt().unwrap();
        println!("secp256k1 aes_key encrypt len={}", data.len());
        let (buf, data2) = pk1.decrypt_aeskey_data(&data).unwrap();
        assert_eq!(buf.len(), 0);
        assert_eq!(aes_key.as_slice(), data2);

        let encrypt_len = data.len();
        data.resize(1024, 0);
        let mut output = vec![0; 48];
        let (buf, size) = pk1.decrypt_aeskey(&data, &mut output).unwrap();
        assert_eq!(buf.len(), 1024 - encrypt_len);
        assert_eq!(aes_key.as_slice(), &output[0..size]);
    }

    fn rsa_private_key_crypto(bits: usize) {
        let pk1 = PrivateKey::generate_rsa(bits).unwrap();
        let (aes_key, data) = pk1.public().gen_aeskey_and_encrypt().unwrap();
        let (buf, data2) = pk1.decrypt_aeskey_data(&data).unwrap();
        assert_eq!(buf.len(), 0);
        assert_eq!(aes_key.as_slice(), data2);
    }
}

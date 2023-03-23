use cyfs_base::*;

use base58::{FromBase58, ToBase58};
use hmac::Hmac;


pub struct AesKeyHelper {}

impl AesKeyHelper {
    pub fn gen(password: &str, device_id: &DeviceId) -> AesKey {
        let salt = device_id.to_string();
        let mut seed = vec![0u8; 64];
        pbkdf2::pbkdf2::<Hmac<sha2::Sha512>>(password.as_bytes(), salt.as_bytes(), 4096, &mut seed);

        seed.resize(48, 0);
        AesKey::from(seed)
    }

    pub fn encrypt_device_id(aes_key: &AesKey, device_id: &DeviceId) -> String {
        let mut bytes = device_id.object_id().as_slice().to_owned();
        let len = bytes.len();
        let pad_len = AesKey::padded_len(bytes.len());
        println!("pad len={}", pad_len);

        bytes.resize(pad_len, 0);

        let encrypt_len = aes_key.inplace_encrypt(&mut bytes, len).unwrap();
        assert_eq!(encrypt_len, bytes.len());

        bytes.to_base58()
    }

    pub fn verify_device_id(
        aes_key: &AesKey,
        device_id: &DeviceId,
        encrypt_device_id: &str,
    ) -> BuckyResult<()> {
        let mut bytes = encrypt_device_id.from_base58().map_err(|e| {
            let msg = format!(
                "convert base58 str to device_id buf failed, str={}, {:?}",
                encrypt_device_id, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let len = bytes.len();
        let decrypt_len = aes_key.inplace_decrypt(&mut bytes, len).map_err(|e| {
            let msg = format!(
                "decrypt device_id buf failed, str={}, {}",
                encrypt_device_id, e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::Unmatch, msg)
        })?;
        bytes.resize(decrypt_len, 0);

        if bytes != device_id.object_id().as_slice() {
            let msg = format!(
                "decrypt device_id to buf but unmatched!, except={:?}, got={:?}",
                device_id.object_id().as_slice(),
                bytes
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Unmatch, msg));
        }

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test() {
        let password = "123456";
        let device_id = DeviceId::default();

        let key = AesKeyHelper::gen(password, &device_id);
        println!("key: {}", key.to_base58());

        let encrypt_device_id = AesKeyHelper::encrypt_device_id(&key, &device_id);
        AesKeyHelper::verify_device_id(&key, &device_id, &encrypt_device_id).unwrap();

        let key2 = AesKeyHelper::gen("1234", &device_id);
        println!("key2: {}", key2.to_base58());
        let e = AesKeyHelper::verify_device_id(&key2, &device_id, &encrypt_device_id).unwrap_err();
        assert_eq!(e.code(), BuckyErrorCode::Unmatch);
    }
}

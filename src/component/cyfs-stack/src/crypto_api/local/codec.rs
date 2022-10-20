use crate::zone::*;
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::*;

use std::borrow::Cow;

pub struct CryptoCodec {
    bdt_stack: StackGuard,
    zone_manager: ZoneManagerRef,
}

impl CryptoCodec {
    pub(crate) fn new(zone_manager: ZoneManagerRef, bdt_stack: StackGuard) -> Self {
        Self {
            bdt_stack,
            zone_manager,
        }
    }

    async fn get_pk(&self, flags: u32) -> BuckyResult<Option<Cow<PublicKey>>> {
        if flags & CRYPTO_REQUEST_FLAG_CRYPT_BY_OWNER != 0 {
            let info = self.zone_manager.get_current_info().await?;
            let pk = info.owner.public_key();
            if pk.is_none() {
                return Ok(None);
            }

            match pk.unwrap() {
                PublicKeyRef::Single(pk) => Ok(Some(Cow::Owned(pk.to_owned()))),
                PublicKeyRef::MN((_, list)) => {
                    if list.len() == 0 {
                        Ok(None)
                    } else {
                        Ok(Some(Cow::Owned(list[0].to_owned())))
                    }
                }
            }
        } else if flags & CRYPTO_REQUEST_FLAG_CRYPT_BY_DEVICE != 0 {
            Ok(Some(Cow::Borrowed(self.bdt_stack.keystore().public_key())))
        } else {
            let msg = format!(
                "invalid encrypt data flags, encrypt data only support by device/owner! flags={}",
                flags
            );
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
        }
    }

    async fn get_sk(&self, flags: u32) -> BuckyResult<Option<Cow<PrivateKey>>> {
        if flags & CRYPTO_REQUEST_FLAG_CRYPT_BY_OWNER != 0 {
            Ok(None)
        } else if flags & CRYPTO_REQUEST_FLAG_CRYPT_BY_DEVICE != 0 {
            Ok(Some(Cow::Borrowed(self.bdt_stack.keystore().private_key())))
        } else {
            let msg = format!(
                "invalid decrypt data flags, decrypt data only support by device/owner! flags={}",
                flags
            );
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
        }
    }

    pub async fn encrypt_data(
        &self,
        req: CryptoEncryptDataInputRequest,
    ) -> BuckyResult<CryptoEncryptDataInputResponse> {
        let pk = self.get_pk(req.flags).await?;
        if pk.is_none() {
            let msg = format!(
                "encrypt data but target public key not found! flags={}",
                req.flags
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let pk = pk.unwrap();

        match req.encrypt_type {
            CryptoEncryptType::EncryptData => {
                if req.data.is_none() || req.data.as_ref().unwrap().is_empty() {
                    let msg = format!("encrypt data but data is empty! flags={}", req.flags);
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }

                let data = req.data.unwrap();
                let result = pk.encrypt_data(&data)?;
                Ok(CryptoEncryptDataInputResponse {
                    aes_key: None,
                    result,
                })
            }
            CryptoEncryptType::GenAESKeyAndEncrypt => {
                if req.data.is_some() && req.data.as_ref().unwrap().len() > 0 {
                    let msg = format!("encrypt data with GenAESKeyAndEncrypt not supports any data buf! flags={}, data={}", 
                        req.flags, req.data.as_ref().unwrap().len());
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                }

                let (aes_key, result) = pk.gen_aeskey_and_encrypt()?;
                Ok(CryptoEncryptDataInputResponse {
                    aes_key: Some(aes_key),
                    result,
                })
            }
        }
    }

    pub async fn decrypt_data(
        &self,
        req: CryptoDecryptDataInputRequest,
    ) -> BuckyResult<CryptoDecryptDataInputResponse> {
        if req.data.is_empty() {
            let msg = format!("decrypt data but data is empty! flags={}", req.flags);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        if req.flags & CRYPTO_REQUEST_FLAG_CRYPT_BY_DEVICE != 0 {
            let sk = self.get_sk(req.flags).await?;
            if sk.is_none() {
                let msg = format!(
                    "encrypt data but target private key not found! flags={}",
                    req.flags
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }

            let sk = sk.unwrap();
            let data = match req.decrypt_type {
                CryptoDecryptType::DecryptData => sk.decrypt_data(&req.data)?,
                CryptoDecryptType::DecryptAESKey => {
                    let (_, data) = sk.decrypt_aeskey_data(&req.data)?;
                    data
                }
            };

            Ok(CryptoDecryptDataInputResponse {
                result: DecryptDataResult::Decrypted,
                data,
            })
        } else if req.flags & CRYPTO_REQUEST_FLAG_CRYPT_BY_OWNER != 0 {
            Ok(CryptoDecryptDataInputResponse {
                result: DecryptDataResult::Pending,
                data: vec![],
            })
        } else {
            let msg = format!(
                "invalid decrypt data flags, decrypt data only support by device/owner! flags={}",
                req.flags
            );
            error!("{}", msg);
            Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
        }
    }
}

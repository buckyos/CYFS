use crate::bip32::ExtendedPrivateKey;
use crate::path::*;
use crate::seed::Seed;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, PrivateKey};

use bip39::{Language, Mnemonic};
use memzero::Memzero;
use std::fmt;

pub struct CyfsSeedKeyBip {
    seed: Memzero<[u8; 64]>,
}

impl fmt::Debug for CyfsSeedKeyBip {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[Protected CyfsSeedKeyBip]")
    }
}

impl CyfsSeedKeyBip {
    pub fn fix_mnemonic(mnemonic: &str) -> BuckyResult<String> {
        let words: Vec<&str> = mnemonic.split(" ").collect();
        if words.len() != 12 {
            let msg = format!("invalid mnemonic words: len={}", words.len());
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let words: Vec<&str> = words.iter().map(|word| word.trim()).collect();
        let mnemonic = words.join(" ");

        Ok(mnemonic)
    }

    pub fn from_mnemonic(mnemonic: &str, password: Option<&str>) -> BuckyResult<Self> {
        let mnemonic = Self::fix_mnemonic(mnemonic)?;

        let mnemonic = Mnemonic::parse_in_normalized(Language::English, mnemonic.as_str())
            .map_err(|e| {
                let msg = format!("invalid mnemonic: err={}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

        let password = password.unwrap_or("");

        let seed_key = Seed::new(&mnemonic, password);

        // 64bytes
        let buf: [u8; 64] = seed_key
            .as_bytes()
            .try_into()
            .expect("invalid seed key length!");

        let seed: Memzero<[u8; 64]> = Memzero::<[u8; 64]>::from(buf);
        Ok(Self { seed })
    }

    pub fn from_private_key(private_key: &str, people_id: &str) -> BuckyResult<Self> {
        // device的密钥使用peopleId作为password
        let seed_key = Seed::new_from_private_key(private_key, people_id);

        // 64bytes
        let buf: [u8; 64] = seed_key
            .as_bytes()
            .try_into()
            .expect("invalid seed key length!");

        let seed: Memzero<[u8; 64]> = Memzero::<[u8; 64]>::from(buf);
        Ok(Self { seed })
    }

    pub fn from_string(s: &str, password: Option<&str>) -> BuckyResult<Self> {
        let password = password.unwrap_or("");

        let seed_key = Seed::new_from_string(s, password);

        // 64bytes
        let buf: [u8; 64] = seed_key
            .as_bytes()
            .try_into()
            .expect("invalid seed key length!");

        let seed: Memzero<[u8; 64]> = Memzero::<[u8; 64]>::from(buf);
        Ok(Self { seed })
    }

    pub fn sub_key(&self, path: &CyfsChainBipPath) -> BuckyResult<PrivateKey> {
        let path = path.to_string();
        debug!("will derive by path={}", path);

        let epk = ExtendedPrivateKey::derive(self.seed.as_ref(), path.as_str())?;
        Ok(epk.into())
    }

    // 直接从path来生成子密钥, 对path合法性不做检测
    pub fn sub_key_direct_by_path(&self, path: &str) -> BuckyResult<PrivateKey> {
        debug!("will derive direct by path={}", path);

        let epk = ExtendedPrivateKey::derive(self.seed.as_ref(), path)?;
        Ok(epk.into())
    }
}

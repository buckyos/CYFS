use super::output_request::*;
use crate::base::*;
use crate::non::*;
use cyfs_base::*;

use std::fmt;

#[derive(Debug, Clone)]
pub struct CryptoInputRequestCommon {
    // 请求路径，可为空
    pub req_path: Option<String>,

    pub source: RequestSourceInfo,

    // 用以默认行为
    pub target: Option<ObjectId>,

    pub flags: u32,
}

impl fmt::Display for CryptoInputRequestCommon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "req_path: {:?}", self.req_path)?;
        write!(f, ", {}", self.source)?;

        if let Some(target) = &self.target {
            write!(f, ", target: {}", target.to_string())?;
        }

        write!(f, ", flags: {}", self.flags)?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CryptoSignObjectInputRequest {
    pub common: CryptoInputRequestCommon,

    pub object: NONObjectInfo,

    pub flags: u32,
}

impl fmt::Display for CryptoSignObjectInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;
        write!(f, ", object: {}", self.object)?;

        write!(f, ", flags: {}", self.flags)
    }
}

pub type CryptoSignObjectInputResponse = CryptoSignObjectOutputResponse;

#[derive(Debug, Clone)]
pub struct CryptoVerifyObjectInputRequest {
    pub common: CryptoInputRequestCommon,

    // 校验的签名位置
    pub sign_type: VerifySignType,

    // 被校验对象
    pub object: NONObjectInfo,

    // 签名来源对象
    pub sign_object: VerifyObjectType,
}

impl fmt::Display for CryptoVerifyObjectInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        write!(f, ", object: {}", self.object)?;
        write!(f, ", sign_type: {:?}", self.sign_type)?;
        write!(f, ", sign_object: {:?}", self.sign_object)
    }
}

pub type CryptoVerifyObjectInputResponse = CryptoVerifyObjectOutputResponse;

#[derive(Debug, Clone)]
pub struct CryptoEncryptDataInputRequest {
    pub common: CryptoInputRequestCommon,

    pub encrypt_type: CryptoEncryptType,

    pub data: Option<Vec<u8>>,

    pub flags: u32,
}

impl CryptoEncryptDataInputRequest {
    pub fn data_len(&self) -> usize {
        match &self.data {
            Some(data) => data.len(),
            None => 0,
        }
    }
}

impl fmt::Display for CryptoEncryptDataInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        write!(f, ", encrypt_type: {}", self.encrypt_type.as_str())?;
        write!(
            f,
            ", data: {}",
            match &self.data {
                Some(data) => data.len(),
                None => 0,
            }
        )?;
        write!(f, ", flags: {}", self.flags)
    }
}

pub type CryptoEncryptDataInputResponse = CryptoEncryptDataOutputResponse;

#[derive(Debug, Clone)]
pub struct CryptoDecryptDataInputRequest {
    pub common: CryptoInputRequestCommon,

    pub decrypt_type: CryptoDecryptType,

    pub data: Vec<u8>,

    pub flags: u32,
}

impl fmt::Display for CryptoDecryptDataInputRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "common: {}", self.common)?;

        write!(f, ", decrypt_type: {}", self.decrypt_type.as_str())?;
        write!(f, ", data: {}", self.data.len(),)?;
        write!(f, ", flags: {}", self.flags)
    }
}

pub type CryptoDecryptDataInputResponse = CryptoDecryptDataOutputResponse;

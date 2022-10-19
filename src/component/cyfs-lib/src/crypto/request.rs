use super::output_request::*;

pub type CryptoRequestCommon = CryptoOutputRequestCommon;
pub type CryptoSignObjectRequest = CryptoSignObjectOutputRequest;
pub type CryptoSignObjectResponse = CryptoSignObjectOutputResponse;
pub type CryptoVerifyObjectRequest = CryptoVerifyObjectOutputRequest;
pub type CryptoVerifyObjectResponse = CryptoVerifyObjectOutputResponse;

pub type CryptoEncryptDataRequest = CryptoEncryptDataOutputRequest;
pub type CryptoEncryptDataResponse = CryptoEncryptDataOutputResponse;
pub type CryptoDecryptDataRequest = CryptoDecryptDataOutputRequest;
pub type CryptoDecryptDataResponse = CryptoDecryptDataOutputResponse;
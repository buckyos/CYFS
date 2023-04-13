use crate::codec as cyfs_base;
use crate::*;

use int_enum::IntEnum;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::error::Error;
use std::fmt::{self, Debug, Display};
use std::io::ErrorKind;
// use std::process::{ExitCode, Termination};

// The built-in Error Code range of the system [BUCKY_SYSTEM_ERROR_CODE_START, BUCKY_SYSTEM_ERROR_CODE_END)
pub const BUCKY_SYSTEM_ERROR_CODE_START: u16 = 0;
pub const BUCKY_SYSTEM_ERROR_CODE_END: u16 = 5000;

// ERROR CODE range of MetaChain
// [BUCKY_META_ERROR_CODE_START, BUCKY_META_ERROR_CODE_END)
pub const BUCKY_META_ERROR_CODE_START: u16 = 5000;
pub const BUCKY_META_ERROR_CODE_END: u16 = 6000;

pub const BUCKY_META_ERROR_CODE_MAX: u16 =
    BUCKY_META_ERROR_CODE_END - BUCKY_META_ERROR_CODE_START - 1;

// The scope of the error code of the application ext(DEC)
// [BUCKY_DEC_ERROR_CODE_START, BUCKY_DEC_ERROR_CODE_END)
pub const BUCKY_DEC_ERROR_CODE_START: u16 = 15000;
pub const BUCKY_DEC_ERROR_CODE_END: u16 = u16::MAX;

// The maximum value of CODE in DEC ERROR (Code)
pub const BUCKY_DEC_ERROR_CODE_MAX: u16 = BUCKY_DEC_ERROR_CODE_END - BUCKY_DEC_ERROR_CODE_START;

pub fn is_system_error_code(code: u16) -> bool {
    code < BUCKY_SYSTEM_ERROR_CODE_END
}

pub fn is_meta_error_code(code: u16) -> bool {
    code >= BUCKY_META_ERROR_CODE_START && code < BUCKY_META_ERROR_CODE_END
}

pub fn is_dec_error_code(code: u16) -> bool {
    code >= BUCKY_DEC_ERROR_CODE_START
}

// Cyfs's error definition
#[repr(u16)]
#[derive(
    Debug, Clone, Copy, Eq, IntEnum, PartialEq, RawEncode, RawDecode, Serialize, Deserialize,
)]
pub enum BuckySystemErrorCode {
    Ok = 0,

    Failed = 1,
    InvalidParam = 2,
    Timeout = 3,
    NotFound = 4,
    AlreadyExists = 5,
    NotSupport = 6,
    ErrorState = 7,
    InvalidFormat = 8,
    Expired = 9,
    OutOfLimit = 10,
    InternalError = 11,

    PermissionDenied = 12,
    ConnectionRefused = 13,
    ConnectionReset = 14,
    ConnectionAborted = 15,
    NotConnected = 16,
    AddrInUse = 18,
    AddrNotAvailable = 19,
    Interrupted = 20,
    InvalidInput = 21,
    InvalidData = 22,
    WriteZero = 23,
    UnexpectedEof = 24,
    BrokenPipe = 25,
    WouldBlock = 26,

    UnSupport = 27,
    Unmatch = 28,
    ExecuteError = 29,
    Reject = 30,
    Ignored = 31,
    InvalidSignature = 32,
    AlreadyExistsAndSignatureMerged = 33,
    TargetNotFound = 34,
    Aborted = 35,

    ConnectFailed = 40,
    ConnectInterZoneFailed = 41,
    InnerPathNotFound = 42,
    RangeNotSatisfiable = 43,
    UserCanceled = 44, 
    Conflict = 50,

    OutofSessionLimit = 60,

    Redirect = 66,

    MongoDBError = 99,
    SqliteError = 100,
    UrlError = 101,
    ZipError = 102,
    HttpError = 103,
    JsonError = 104,
    HexError = 105,
    RsaError = 106,
    CryptoError = 107,
    MpscSendError = 108,
    MpscRecvError = 109,
    IoError = 110,
    NetworkError = 111,

    CodeError = 250, //TODO: cyfs-base的Code应该和BuckyErrorCode整合，现在先搞个特殊Type让能编过
    UnknownBdtError = 253,
    UnknownIOError = 254,
    Unknown = 255,

    Pending = 256,
    NotChange = 257,

    NotMatch = 258,
    NotImplement = 259,
    NotInit = 260,
    ParseError = 261,
    NotHandled = 262,

    // 在system error code里面，meta_error默认值都取值5000
    MetaError = 5000,

    // 在system error code里面，dec_error默认都是取值15000
    DecError = 15000,
}

impl Into<u16> for BuckySystemErrorCode {
    fn into(self) -> u16 {
        unsafe { std::mem::transmute(self as u16) }
    }
}

impl From<u16> for BuckySystemErrorCode {
    fn from(code: u16) -> Self {
        match Self::from_int(code) {
            Ok(code) => code,
            Err(e) => {
                error!("unknown system error code: {} {}", code, e);
                if is_dec_error_code(code) {
                    Self::DecError
                } else if is_meta_error_code(code) {
                    Self::MetaError
                } else {
                    Self::Unknown
                }
            }
        }
    }
}

// BuckyErrorCode的兼容性定义
#[repr(u16)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, RawEncode, RawDecode)]
pub enum BuckyErrorCode {
    Ok,

    Failed,
    InvalidParam,
    Timeout,
    NotFound,
    AlreadyExists,
    NotSupport,
    ErrorState,
    InvalidFormat,
    Expired,
    OutOfLimit,
    InternalError,

    PermissionDenied,
    ConnectionRefused,
    ConnectionReset,
    ConnectionAborted,
    NotConnected,
    AddrInUse,
    AddrNotAvailable,
    Interrupted,
    InvalidInput,
    InvalidData,
    WriteZero,
    UnexpectedEof,
    BrokenPipe,
    WouldBlock,

    UnSupport,
    Unmatch,
    ExecuteError,
    Reject,
    Ignored,
    InvalidSignature,
    AlreadyExistsAndSignatureMerged,
    TargetNotFound,
    Aborted,

    ConnectFailed,
    ConnectInterZoneFailed,
    InnerPathNotFound,
    RangeNotSatisfiable,
    UserCanceled,

    Conflict,

    OutofSessionLimit,

    Redirect,

    MongoDBError,
    SqliteError,
    UrlError,
    ZipError,
    HttpError,
    JsonError,
    HexError,
    RsaError,
    CryptoError,
    MpscSendError,
    MpscRecvError,
    IoError,
    NetworkError,

    CodeError, //TODO: cyfs-base的Code应该和BuckyErrorCode整合，现在先搞个特殊Type让能编过
    UnknownBdtError,
    UnknownIOError,
    Unknown,

    Pending,
    NotChange,

    NotMatch,
    NotImplement,
    NotInit,

    ParseError,
    NotHandled,

    // meta chain的error段，取值范围是[0, BUCKY_META_ERROR_CODE_MAX)
    MetaError(u16),

    // DEC自定义error，取值范围是[0, BUCKY_DEC_ERROR_CODE_MAX)
    DecError(u16),
}

impl Into<BuckySystemErrorCode> for BuckyErrorCode {
    fn into(self) -> BuckySystemErrorCode {
        match self {
            Self::Ok => BuckySystemErrorCode::Ok,

            Self::Failed => BuckySystemErrorCode::Failed,
            Self::InvalidParam => BuckySystemErrorCode::InvalidParam,
            Self::Timeout => BuckySystemErrorCode::Timeout,
            Self::NotFound => BuckySystemErrorCode::NotFound,
            Self::AlreadyExists => BuckySystemErrorCode::AlreadyExists,
            Self::NotSupport => BuckySystemErrorCode::NotSupport,
            Self::ErrorState => BuckySystemErrorCode::ErrorState,
            Self::InvalidFormat => BuckySystemErrorCode::InvalidFormat,
            Self::Expired => BuckySystemErrorCode::Expired,
            Self::OutOfLimit => BuckySystemErrorCode::OutOfLimit,
            Self::InternalError => BuckySystemErrorCode::InternalError,

            Self::PermissionDenied => BuckySystemErrorCode::PermissionDenied,
            Self::ConnectionRefused => BuckySystemErrorCode::ConnectionRefused,
            Self::ConnectionReset => BuckySystemErrorCode::ConnectionReset,
            Self::ConnectionAborted => BuckySystemErrorCode::ConnectionAborted,
            Self::NotConnected => BuckySystemErrorCode::NotConnected,
            Self::AddrInUse => BuckySystemErrorCode::AddrInUse,
            Self::AddrNotAvailable => BuckySystemErrorCode::AddrNotAvailable,
            Self::Interrupted => BuckySystemErrorCode::Interrupted,
            Self::InvalidInput => BuckySystemErrorCode::InvalidInput,
            Self::InvalidData => BuckySystemErrorCode::InvalidData,
            Self::WriteZero => BuckySystemErrorCode::WriteZero,
            Self::UnexpectedEof => BuckySystemErrorCode::UnexpectedEof,
            Self::BrokenPipe => BuckySystemErrorCode::BrokenPipe,
            Self::WouldBlock => BuckySystemErrorCode::WouldBlock,

            Self::UnSupport => BuckySystemErrorCode::UnSupport,
            Self::Unmatch => BuckySystemErrorCode::Unmatch,
            Self::ExecuteError => BuckySystemErrorCode::ExecuteError,
            Self::Reject => BuckySystemErrorCode::Reject,
            Self::Ignored => BuckySystemErrorCode::Ignored,
            Self::InvalidSignature => BuckySystemErrorCode::InvalidSignature,
            Self::AlreadyExistsAndSignatureMerged => {
                BuckySystemErrorCode::AlreadyExistsAndSignatureMerged
            }
            Self::TargetNotFound => BuckySystemErrorCode::TargetNotFound,
            Self::Aborted => BuckySystemErrorCode::Aborted,

            Self::ConnectFailed => BuckySystemErrorCode::ConnectFailed,
            Self::ConnectInterZoneFailed => BuckySystemErrorCode::ConnectInterZoneFailed,
            Self::InnerPathNotFound => BuckySystemErrorCode::InnerPathNotFound,
            Self::RangeNotSatisfiable => BuckySystemErrorCode::RangeNotSatisfiable,
            Self::UserCanceled => BuckySystemErrorCode::UserCanceled, 
            Self::Conflict => BuckySystemErrorCode::Conflict,

            Self::OutofSessionLimit => BuckySystemErrorCode::OutofSessionLimit,
            Self::Redirect => BuckySystemErrorCode::Redirect,

            Self::MongoDBError => BuckySystemErrorCode::MongoDBError,
            Self::SqliteError => BuckySystemErrorCode::SqliteError,
            Self::UrlError => BuckySystemErrorCode::UrlError,
            Self::ZipError => BuckySystemErrorCode::ZipError,
            Self::HttpError => BuckySystemErrorCode::HttpError,
            Self::JsonError => BuckySystemErrorCode::JsonError,
            Self::HexError => BuckySystemErrorCode::RsaError,
            Self::RsaError => BuckySystemErrorCode::InternalError,
            Self::CryptoError => BuckySystemErrorCode::CryptoError,
            Self::MpscSendError => BuckySystemErrorCode::MpscSendError,
            Self::MpscRecvError => BuckySystemErrorCode::MpscRecvError,
            Self::IoError => BuckySystemErrorCode::IoError,
            Self::NetworkError => BuckySystemErrorCode::NetworkError,

            Self::CodeError => BuckySystemErrorCode::CodeError,
            Self::UnknownBdtError => BuckySystemErrorCode::UnknownBdtError,
            Self::UnknownIOError => BuckySystemErrorCode::UnknownIOError,
            Self::Unknown => BuckySystemErrorCode::Unknown,

            Self::Pending => BuckySystemErrorCode::Pending,
            Self::NotChange => BuckySystemErrorCode::NotChange,

            Self::NotMatch => BuckySystemErrorCode::NotMatch,
            Self::NotImplement => BuckySystemErrorCode::NotImplement,
            Self::NotInit => BuckySystemErrorCode::NotInit,

            Self::ParseError => BuckySystemErrorCode::ParseError,
            Self::NotHandled => BuckySystemErrorCode::NotHandled,

            Self::MetaError(_) => BuckySystemErrorCode::MetaError,
            Self::DecError(_) => BuckySystemErrorCode::DecError,
        }
    }
}

impl Into<BuckyErrorCode> for BuckySystemErrorCode {
    fn into(self) -> BuckyErrorCode {
        match self {
            Self::Ok => BuckyErrorCode::Ok,

            Self::Failed => BuckyErrorCode::Failed,
            Self::InvalidParam => BuckyErrorCode::InvalidParam,
            Self::Timeout => BuckyErrorCode::Timeout,
            Self::NotFound => BuckyErrorCode::NotFound,
            Self::AlreadyExists => BuckyErrorCode::AlreadyExists,
            Self::NotSupport => BuckyErrorCode::NotSupport,
            Self::ErrorState => BuckyErrorCode::ErrorState,
            Self::InvalidFormat => BuckyErrorCode::InvalidFormat,
            Self::Expired => BuckyErrorCode::Expired,
            Self::OutOfLimit => BuckyErrorCode::OutOfLimit,
            Self::InternalError => BuckyErrorCode::InternalError,

            Self::PermissionDenied => BuckyErrorCode::PermissionDenied,
            Self::ConnectionRefused => BuckyErrorCode::ConnectionRefused,
            Self::ConnectionReset => BuckyErrorCode::ConnectionReset,
            Self::ConnectionAborted => BuckyErrorCode::ConnectionAborted,
            Self::NotConnected => BuckyErrorCode::NotConnected,
            Self::AddrInUse => BuckyErrorCode::AddrInUse,
            Self::AddrNotAvailable => BuckyErrorCode::AddrNotAvailable,
            Self::Interrupted => BuckyErrorCode::Interrupted,
            Self::InvalidInput => BuckyErrorCode::InvalidInput,
            Self::InvalidData => BuckyErrorCode::InvalidData,
            Self::WriteZero => BuckyErrorCode::WriteZero,
            Self::UnexpectedEof => BuckyErrorCode::UnexpectedEof,
            Self::BrokenPipe => BuckyErrorCode::BrokenPipe,
            Self::WouldBlock => BuckyErrorCode::WouldBlock,

            Self::UnSupport => BuckyErrorCode::UnSupport,
            Self::Unmatch => BuckyErrorCode::Unmatch,
            Self::ExecuteError => BuckyErrorCode::ExecuteError,
            Self::Reject => BuckyErrorCode::Reject,
            Self::Ignored => BuckyErrorCode::Ignored,
            Self::InvalidSignature => BuckyErrorCode::InvalidSignature,
            Self::AlreadyExistsAndSignatureMerged => {
                BuckyErrorCode::AlreadyExistsAndSignatureMerged
            }
            Self::TargetNotFound => BuckyErrorCode::TargetNotFound,
            Self::Aborted => BuckyErrorCode::Aborted,

            Self::ConnectFailed => BuckyErrorCode::ConnectFailed,
            Self::ConnectInterZoneFailed => BuckyErrorCode::ConnectInterZoneFailed,
            Self::InnerPathNotFound => BuckyErrorCode::InnerPathNotFound,
            Self::RangeNotSatisfiable => BuckyErrorCode::RangeNotSatisfiable,
            Self::UserCanceled => BuckyErrorCode::UserCanceled, 

            Self::Conflict => BuckyErrorCode::Conflict,

            Self::OutofSessionLimit => BuckyErrorCode::OutofSessionLimit,

            Self::Redirect => BuckyErrorCode::Redirect,
            
            Self::MongoDBError => BuckyErrorCode::MongoDBError,
            Self::SqliteError => BuckyErrorCode::SqliteError,
            Self::UrlError => BuckyErrorCode::UrlError,
            Self::ZipError => BuckyErrorCode::ZipError,
            Self::HttpError => BuckyErrorCode::HttpError,
            Self::JsonError => BuckyErrorCode::JsonError,
            Self::HexError => BuckyErrorCode::RsaError,
            Self::RsaError => BuckyErrorCode::InternalError,
            Self::CryptoError => BuckyErrorCode::CryptoError,
            Self::MpscSendError => BuckyErrorCode::MpscSendError,
            Self::MpscRecvError => BuckyErrorCode::MpscRecvError,
            Self::IoError => BuckyErrorCode::IoError,
            Self::NetworkError => BuckyErrorCode::NetworkError,

            Self::CodeError => BuckyErrorCode::CodeError,
            Self::UnknownBdtError => BuckyErrorCode::UnknownBdtError,
            Self::UnknownIOError => BuckyErrorCode::UnknownIOError,
            Self::Unknown => BuckyErrorCode::Unknown,

            Self::Pending => BuckyErrorCode::Pending,
            Self::NotChange => BuckyErrorCode::NotChange,

            Self::NotMatch => BuckyErrorCode::NotMatch,
            Self::NotImplement => BuckyErrorCode::NotImplement,
            Self::NotInit => BuckyErrorCode::NotInit,

            Self::ParseError => BuckyErrorCode::ParseError,
            Self::NotHandled => BuckyErrorCode::NotHandled,

            Self::MetaError => BuckyErrorCode::MetaError(0),
            Self::DecError => BuckyErrorCode::DecError(0),
        }
    }
}

impl Into<u32> for BuckyErrorCode {
    fn into(self) -> u32 {
        let v: u16 = self.into();
        v as u32
    }
}

impl Into<i32> for BuckyErrorCode {
    fn into(self) -> i32 {
        let v: u16 = self.into();
        v as i32
    }
}

impl Into<u16> for BuckyErrorCode {
    fn into(self) -> u16 {
        match self {
            Self::MetaError(mut v) => {
                if v > BUCKY_META_ERROR_CODE_MAX {
                    error!("meta error code out of limit: {}", v);
                    v = BUCKY_META_ERROR_CODE_MAX;
                }

                BUCKY_META_ERROR_CODE_START + v
            }
            Self::DecError(mut v) => {
                if v > BUCKY_DEC_ERROR_CODE_MAX {
                    error!("dec error code out of limit: {}", v);
                    v = BUCKY_DEC_ERROR_CODE_MAX;
                }

                BUCKY_DEC_ERROR_CODE_START + v
            }
            _ => Into::<BuckySystemErrorCode>::into(self).into(),
        }
    }
}

impl From<u16> for BuckyErrorCode {
    fn from(code: u16) -> Self {
        if is_system_error_code(code) {
            BuckySystemErrorCode::from(code).into()
        } else if is_meta_error_code(code) {
            let code = code - BUCKY_META_ERROR_CODE_START;
            Self::MetaError(code)
        } else if is_dec_error_code(code) {
            let code = code - BUCKY_DEC_ERROR_CODE_START;
            Self::DecError(code)
        } else {
            error!("unknown error code: {}", code);
            Self::Unknown
        }
    }
}

impl From<u32> for BuckyErrorCode {
    fn from(code: u32) -> Self {
        if code < u16::MAX as u32 {
            Self::from(code as u16)
        } else {
            error!("u32 error code out of u16 limit: {}", code);
            Self::Unknown
        }
    }
}

impl Display for BuckyErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_u16())
    }
}

impl Serialize for BuckyErrorCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.as_u16())
    }
}

struct BuckyErrorCodeVisitor {}
impl<'de> Visitor<'de> for BuckyErrorCodeVisitor {
    type Value = BuckyErrorCode;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("u16")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if v < u16::MAX as u64 {
            Ok(BuckyErrorCode::from(v as u16))
        } else {
            error!("invalid BuckyErrorCode int value: {}", v);
            Ok(BuckyErrorCode::Unknown)
        }
    }
}

impl<'de> Deserialize<'de> for BuckyErrorCode {
    fn deserialize<D>(deserializer: D) -> Result<BuckyErrorCode, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_u16(BuckyErrorCodeVisitor {})
    }
}

impl BuckyErrorCode {
    pub fn as_u8(&self) -> u8 {
        let v: u16 = self.clone().into();
        v as u8
    }

    pub fn into_u8(self) -> u8 {
        let v: u16 = self.into();
        v as u8
    }

    pub fn as_u16(&self) -> u16 {
        self.clone().into()
    }

    pub fn into_u16(self) -> u16 {
        self.into()
    }

    // 判断是不是DecError
    pub fn is_meta_error(&self) -> bool {
        match *self {
            Self::MetaError(_) => true,
            _ => false,
        }
    }

    // 判断是不是DecError
    pub fn is_dec_error(&self) -> bool {
        match *self {
            Self::DecError(_) => true,
            _ => false,
        }
    }
}

// BuckyErrorCode的标准定义
pub enum BuckyErrorCodeEx {
    System(BuckySystemErrorCode),
    MetaError(u16),
    DecError(u16),
}

impl Into<BuckyErrorCode> for BuckyErrorCodeEx {
    fn into(self) -> BuckyErrorCode {
        match self {
            Self::System(code) => code.into(),
            Self::MetaError(v) => BuckyErrorCode::MetaError(v),
            Self::DecError(v) => BuckyErrorCode::DecError(v),
        }
    }
}

impl Into<BuckyErrorCodeEx> for BuckyErrorCode {
    fn into(self) -> BuckyErrorCodeEx {
        match self {
            Self::MetaError(v) => BuckyErrorCodeEx::MetaError(v),
            Self::DecError(v) => BuckyErrorCodeEx::DecError(v),
            _ => self.into(),
        }
    }
}

impl Into<BuckyErrorCodeEx> for BuckySystemErrorCode {
    fn into(self) -> BuckyErrorCodeEx {
        BuckyErrorCodeEx::System(self)
    }
}

// 第三方模块和std内部的Errror
#[derive(Debug)]
pub enum BuckyOriginError {
    IoError(std::io::Error),
    SerdeJsonError(serde_json::error::Error),
    HttpError(http_types::Error),
    UrlError(url::ParseError),
    #[cfg(not(target_arch = "wasm32"))]
    ZipError(zip::result::ZipError),
    HttpStatusCodeError(http_types::StatusCode),
    #[cfg(not(target_arch = "wasm32"))]
    SqliteError(rusqlite::Error),
    #[cfg(feature = "sqlx-error")]
    SqlxError(sqlx::Error),
    HexError(hex::FromHexError),
    RsaError(rsa::errors::Error),
    CodeError(u32),
    ParseIntError(std::num::ParseIntError),
    ParseFloatError(std::num::ParseFloatError),
    AddrParseError(std::net::AddrParseError),
    StripPrefixError(std::path::StripPrefixError),
    ParseUtf8Error(std::str::Utf8Error),
    ErrorMsg(String),
}

impl RawEncode for BuckyOriginError {
    fn raw_measure(&self, purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        if cfg!(not(target_arch = "wasm32")) {
            if let BuckyOriginError::ZipError(e) = self {
                let msg = format!("{:?}", e);
                return Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?);
            }

            if let BuckyOriginError::SqliteError(e) = self {
                let msg = format!("{:?}", e);
                return Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?);
            }
        }

        match self {
            BuckyOriginError::IoError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::SerdeJsonError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::HttpError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::UrlError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::HttpStatusCodeError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::HexError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::RsaError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::CodeError(e) => {
                Ok(USize(1).raw_measure(purpose)? + e.raw_measure(purpose)?)
            }
            BuckyOriginError::ParseIntError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::ParseFloatError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::AddrParseError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::StripPrefixError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::ParseUtf8Error(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            BuckyOriginError::ErrorMsg(msg) => {
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            #[cfg(feature = "sqlx-error")]
            BuckyOriginError::SqlxError(e) => {
                let msg = format!("{:?}", e);
                Ok(USize(2).raw_measure(purpose)? + msg.raw_measure(purpose)?)
            }
            _ => Ok(USize(3).raw_measure(purpose)?),
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        if cfg!(not(target_arch = "wasm32")) {
            if let BuckyOriginError::ZipError(e) = self {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                return Ok(buf);
            }

            if let BuckyOriginError::SqliteError(e) = self {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                return Ok(buf);
            }
        }

        match self {
            BuckyOriginError::IoError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::SerdeJsonError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::HttpError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::UrlError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::HttpStatusCodeError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::HexError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::RsaError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::CodeError(e) => {
                let buf = USize(1).raw_encode(buf, purpose)?;
                let buf = e.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::ParseIntError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::ParseFloatError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::AddrParseError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::StripPrefixError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::ParseUtf8Error(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            BuckyOriginError::ErrorMsg(msg) => {
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            #[cfg(feature = "sqlx-error")]
            BuckyOriginError::SqlxError(e) => {
                let msg = format!("{:?}", e);
                let buf = USize(2).raw_encode(buf, purpose)?;
                let buf = msg.raw_encode(buf, purpose)?;
                Ok(buf)
            }
            _ => {
                let buf = USize(3).raw_encode(buf, purpose)?;
                Ok(buf)
            }
        }
    }
}

impl<'de> RawDecode<'de> for BuckyOriginError {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (t, buf) = USize::raw_decode(buf)?;
        return if t.0 == 1 {
            let (code, buf) = u32::raw_decode(buf)?;
            Ok((BuckyOriginError::CodeError(code), buf))
        } else if t.0 == 2 {
            let (msg, buf) = String::raw_decode(buf)?;
            Ok((BuckyOriginError::ErrorMsg(msg), buf))
        } else {
            Ok((BuckyOriginError::ErrorMsg("".to_string()), buf))
        };
    }
}

#[derive(RawEncode, RawDecode)]
pub struct BuckyError {
    code: BuckyErrorCode,
    msg: String,

    origin: Option<BuckyOriginError>,
}

pub type BuckyResult<T> = Result<T, BuckyError>;

// 为BuckyError实现一个可能丢失origin信息的clone
// TODO 改进originError
impl Clone for BuckyError {
    fn clone(&self) -> Self {
        BuckyError::new(self.code(), self.msg())
    }
}

impl BuckyError {
    pub fn new(code: impl Into<BuckyErrorCode>, msg: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            msg: msg.into(),
            origin: None,
        }
    }

    pub fn set_code(&mut self, code: impl Into<BuckyErrorCode>) {
        self.code = code.into();
    }

    pub fn code(&self) -> BuckyErrorCode {
        self.code
    }

    pub fn with_code(mut self, code: impl Into<BuckyErrorCode>) -> Self {
        self.code = code.into();
        self
    }

    pub fn set_msg(&mut self, msg: impl Into<String>) {
        self.msg = msg.into();
    }

    pub fn msg(&self) -> &str {
        self.msg.as_ref()
    }

    pub fn with_msg(mut self, msg: impl Into<String>) -> Self {
        self.msg = msg.into();
        self
    }

    pub fn origin(&self) -> &Option<BuckyOriginError> {
        &self.origin
    }

    pub fn into_origin(self) -> Option<BuckyOriginError> {
        self.origin
    }

    fn format(&self) -> String {
        format!("err: ({:?}, {}, {:?})", self.code, self.msg, self.origin)
    }

    pub fn error_with_log<T>(msg: impl Into<String> + std::fmt::Display) -> BuckyResult<T> {
        error!("{}", msg);

        Err(BuckyError::new(BuckyErrorCode::Failed, msg))
    }
}

impl Error for BuckyError {}

impl Display for BuckyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.format(), f)
    }
}

impl Debug for BuckyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.format(), f)
    }
}

impl BuckyError {
    fn io_error_kind_to_code(kind: std::io::ErrorKind) -> BuckyErrorCode {
        match kind {
            ErrorKind::NotFound => BuckyErrorCode::NotFound,
            ErrorKind::PermissionDenied => BuckyErrorCode::PermissionDenied,
            ErrorKind::ConnectionRefused => BuckyErrorCode::ConnectionRefused,
            ErrorKind::ConnectionReset => BuckyErrorCode::ConnectionReset,
            ErrorKind::ConnectionAborted => BuckyErrorCode::ConnectionAborted,
            ErrorKind::NotConnected => BuckyErrorCode::NotConnected,
            ErrorKind::AddrInUse => BuckyErrorCode::AddrInUse,
            ErrorKind::AddrNotAvailable => BuckyErrorCode::AddrNotAvailable,
            ErrorKind::BrokenPipe => BuckyErrorCode::BrokenPipe,
            ErrorKind::AlreadyExists => BuckyErrorCode::AlreadyExists,
            ErrorKind::WouldBlock => BuckyErrorCode::WouldBlock,
            ErrorKind::InvalidInput => BuckyErrorCode::InvalidInput,
            ErrorKind::InvalidData => BuckyErrorCode::InvalidData,
            ErrorKind::TimedOut => BuckyErrorCode::Timeout,
            ErrorKind::WriteZero => BuckyErrorCode::WriteZero,
            ErrorKind::Interrupted => BuckyErrorCode::Interrupted,
            ErrorKind::UnexpectedEof => BuckyErrorCode::UnexpectedEof,
            _ => BuckyErrorCode::UnknownIOError,
        }
    }

    /*
    fn parse_int_error_kind_to_code(_kind: std::num::IntErrorKind) -> BuckyErrorCode {
        BuckyErrorCode::InvalidFormat
    }
    */

    fn convert_bdt_error_code(code: u32) -> BuckyErrorCode {
        match code {
            _ => BuckyErrorCode::UnknownBdtError,
        }
    }

    fn bucky_error_to_io_error(e: BuckyError) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::Other, e)
    }

    fn io_error_to_bucky_error(e: std::io::Error) -> BuckyError {
        let kind = e.kind();
        if kind == std::io::ErrorKind::Other && e.get_ref().is_some() {
            match e.into_inner().unwrap().downcast::<BuckyError>() {
                Ok(e) => {
                    e.as_ref().clone()
                }
                Err(e) => {
                    BuckyError {
                        code: Self::io_error_kind_to_code(kind),
                        msg: format!("io_error: {}", e),
                        origin: None,
                    }
                }
            }
        } else {
            BuckyError {
                code: Self::io_error_kind_to_code(e.kind()),
                msg: format!("io_error: {}", e),
                origin: Some(BuckyOriginError::IoError(e)),
            }
        }
    }
}

impl From<std::io::Error> for BuckyError {
    fn from(err: std::io::Error) -> BuckyError {
        BuckyError::io_error_to_bucky_error(err)
    }
}

impl From<BuckyError> for std::io::Error {
    fn from(err: BuckyError) -> std::io::Error {
        BuckyError::bucky_error_to_io_error(err)
    }
}

impl From<std::str::Utf8Error> for BuckyError {
    fn from(err: std::str::Utf8Error) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::InvalidFormat,
            msg: format!("io_error: {}", err),
            origin: Some(BuckyOriginError::ParseUtf8Error(err)),
        }
    }
}

impl From<http_types::Error> for BuckyError {
    fn from(err: http_types::Error) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::HttpError,
            msg: format!("http_error: {}", err),
            origin: Some(BuckyOriginError::HttpError(err)),
        }
    }
}

impl From<std::num::ParseIntError> for BuckyError {
    fn from(err: std::num::ParseIntError) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::InvalidFormat,
            msg: format!("parse_int_error: {}", err),
            origin: Some(BuckyOriginError::ParseIntError(err)),
        }
    }
}

impl From<std::num::ParseFloatError> for BuckyError {
    fn from(err: std::num::ParseFloatError) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::InvalidFormat,
            msg: format!("parse_int_error: {}", err),
            origin: Some(BuckyOriginError::ParseFloatError(err)),
        }
    }
}

impl From<std::net::AddrParseError> for BuckyError {
    fn from(err: std::net::AddrParseError) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::InvalidFormat,
            msg: format!("parse_int_error: {}", err),
            origin: Some(BuckyOriginError::AddrParseError(err)),
        }
    }
}

impl From<std::path::StripPrefixError> for BuckyError {
    fn from(err: std::path::StripPrefixError) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::InvalidFormat,
            msg: format!("strip_prefix_error: {}", err),
            origin: Some(BuckyOriginError::StripPrefixError(err)),
        }
    }
}

impl From<async_std::future::TimeoutError> for BuckyError {
    fn from(err: async_std::future::TimeoutError) -> BuckyError {
        BuckyError::new(BuckyErrorCode::Timeout, format!("{}", err))
    }
}

impl From<u32> for BuckyError {
    fn from(err: u32) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::CodeError,
            msg: format!("base_code_error: {}", err),
            origin: Some(BuckyOriginError::CodeError(err)),
        }
    }
}

pub struct CodeError(pub u32, pub String);

impl From<CodeError> for BuckyError {
    fn from(err: CodeError) -> Self {
        BuckyError {
            code: BuckyErrorCode::CodeError,
            msg: err.1,
            origin: Some(BuckyOriginError::CodeError(err.0)),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<rusqlite::Error> for BuckyError {
    fn from(err: rusqlite::Error) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::SqliteError,
            msg: format!("sqlite_error: {}", err),
            origin: Some(BuckyOriginError::SqliteError(err)),
        }
    }
}
#[cfg(feature = "sqlx-error")]
impl From<sqlx::Error> for BuckyError {
    fn from(err: sqlx::Error) -> Self {
        Self {
            code: BuckyErrorCode::SqliteError,
            msg: format!("sqlx error: {}", err),
            origin: Some(BuckyOriginError::SqlxError(err))
        }
    }
}

impl From<serde_json::error::Error> for BuckyError {
    fn from(e: serde_json::error::Error) -> Self {
        BuckyError {
            code: BuckyErrorCode::JsonError,
            msg: format!("json_error: {}", e),
            origin: Some(BuckyOriginError::SerdeJsonError(e)),
        }
    }
}

impl From<http_types::StatusCode> for BuckyError {
    fn from(code: http_types::StatusCode) -> Self {
        BuckyError {
            code: BuckyErrorCode::HttpError,
            msg: format!("http status code: {}", code),
            origin: Some(BuckyOriginError::HttpStatusCodeError(code)),
        }
    }
}

impl From<url::ParseError> for BuckyError {
    fn from(e: url::ParseError) -> Self {
        BuckyError {
            code: BuckyErrorCode::UrlError,
            msg: format!("url_error: {}", e),
            origin: Some(BuckyOriginError::UrlError(e)),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<zip::result::ZipError> for BuckyError {
    fn from(e: zip::result::ZipError) -> Self {
        BuckyError {
            code: BuckyErrorCode::ZipError,
            msg: format!("zip_error: {:?}", e),
            origin: Some(BuckyOriginError::ZipError(e)),
        }
    }
}

impl From<hex::FromHexError> for BuckyError {
    fn from(e: hex::FromHexError) -> Self {
        BuckyError {
            code: BuckyErrorCode::HexError,
            msg: format!("hex error: {}", e),
            origin: Some(BuckyOriginError::HexError(e)),
        }
    }
}

impl From<rsa::errors::Error> for BuckyError {
    fn from(e: rsa::errors::Error) -> Self {
        BuckyError {
            code: BuckyErrorCode::RsaError,
            msg: format!("rsa error: {:?}", e),
            origin: Some(BuckyOriginError::RsaError(e)),
        }
    }
}

impl From<BuckyErrorCode> for BuckyError {
    fn from(code: BuckyErrorCode) -> BuckyError {
        BuckyError {
            code,
            msg: "".to_owned(),
            origin: None,
        }
    }
}

impl From<&str> for BuckyError {
    fn from(msg: &str) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::Unknown,
            msg: msg.to_owned(),
            origin: None,
        }
    }
}

impl From<String> for BuckyError {
    fn from(msg: String) -> BuckyError {
        BuckyError {
            code: BuckyErrorCode::Unknown,
            msg,
            origin: None,
        }
    }
}

impl From<(BuckyErrorCode, &str)> for BuckyError {
    fn from(cm: (BuckyErrorCode, &str)) -> BuckyError {
        BuckyError {
            code: cm.0,
            msg: cm.1.to_owned(),
            origin: None,
        }
    }
}

impl From<(BuckyErrorCode, String)> for BuckyError {
    fn from(cm: (BuckyErrorCode, String)) -> BuckyError {
        BuckyError {
            code: cm.0,
            msg: cm.1,
            origin: None,
        }
    }
}

/*
暂不支持
impl From<&Box<dyn Error>> for BuckyError{
    fn from(err: &Box<dyn Error>) -> BuckyError {
        if err.is::<BuckyError>() {
            let be = err.downcast_ref::<BuckyError>().unwrap();
            BuckyError {
                code: be.code,
                msg: be.msg.clone(),
                origin: be.origin.clone(),
            }
        } else {
            BuckyError {
                code: BuckyErrorCode::Unknown,
                msg: format!("{}", err),
                origin: None,
            }
        }
    }
}
*/

impl From<Box<dyn Error>> for BuckyError {
    fn from(err: Box<dyn Error>) -> BuckyError {
        if err.is::<BuckyError>() {
            let be = err.downcast::<BuckyError>().unwrap();
            *be
        } else {
            BuckyError {
                code: BuckyErrorCode::Unknown,
                msg: format!("{}", err),
                origin: None,
            }
        }
    }
}

impl Into<BuckyErrorCode> for BuckyError {
    fn into(self) -> BuckyErrorCode {
        self.code
    }
}

impl Into<ErrorKind> for BuckyErrorCode {
    fn into(self) -> ErrorKind {
        match self {
            Self::Reject | Self::PermissionDenied => ErrorKind::PermissionDenied,
            Self::NotFound => ErrorKind::NotFound,

            Self::ConnectionReset => ErrorKind::ConnectionReset,
            Self::ConnectionRefused => ErrorKind::ConnectionRefused,
            Self::ConnectionAborted => ErrorKind::ConnectionAborted,
            Self::AddrInUse => ErrorKind::AddrInUse,
            Self::AddrNotAvailable => ErrorKind::AddrNotAvailable,
            Self::NotConnected => ErrorKind::NotConnected,
            Self::AlreadyExists => ErrorKind::AlreadyExists,
            Self::Interrupted => ErrorKind::Interrupted,
            Self::WriteZero => ErrorKind::WriteZero,
            Self::UnexpectedEof => ErrorKind::UnexpectedEof,
            Self::UnSupport => ErrorKind::Unsupported,
            Self::BrokenPipe => ErrorKind::BrokenPipe,
            Self::WouldBlock => ErrorKind::WouldBlock,
            Self::Timeout => ErrorKind::TimedOut,
            Self::OutOfLimit => ErrorKind::OutOfMemory,

            _ => ErrorKind::Other,
        }
    }
}
/*
impl Termination for BuckyError {
    fn report(self) -> ExitCode {
        ExitCode::from(self.code.into_u8())
    }
}
*/
#[cfg(test)]
mod tests {
    use crate::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    struct SimpleBuckyError {
        msg: String,
        code: BuckyErrorCode,
    }

    fn re_error() -> BuckyError {
        BuckyError::from(BuckyErrorCode::Failed)
    }

    fn re_error2() -> BuckyError {
        BuckyError::from(format!("test error"))
    }

    #[test]
    fn test_codec() {
        let code = BuckyErrorCode::ErrorState;
        let v = serde_json::to_string(&code).unwrap();
        println!("error code: {}", v);
        let code2: BuckyErrorCode = serde_json::from_str(&v).unwrap();
        assert_eq!(code2, code);

        let e = SimpleBuckyError {
            code: BuckyErrorCode::InvalidFormat,
            msg: "test".to_owned(),
        };
        let v = serde_json::to_string(&e).unwrap();
        println!("error: {}", v);
        let e2: SimpleBuckyError = serde_json::from_str(&v).unwrap();
        assert_eq!(e, e2);
    }

    #[test]
    fn test_error() {
        let err = re_error();
        assert!(err.code() == BuckyErrorCode::Failed);

        re_error2();

        let user_error = 101;
        let code = BuckyErrorCode::DecError(user_error);
        let value: u16 = code.into();
        let code2 = BuckyErrorCode::from(value);
        assert_eq!(code, code2);

        let user_error = u16::MAX;
        let code = BuckyErrorCode::DecError(user_error);
        let value: u16 = code.into();
        let code2 = BuckyErrorCode::from(value);
        assert_ne!(code, code2);
        let max_code = BuckyErrorCode::DecError(BUCKY_DEC_ERROR_CODE_MAX);
        assert_eq!(max_code, code2);

        let code = BuckyErrorCode::Unknown;
        let value: u16 = code.into();
        let code2 = BuckyErrorCode::from(value);
        assert_eq!(code, code2);
    }

    
    #[test]
    fn test_io_error() {
        let err =  BuckyError::new(BuckyErrorCode::AddrInUse, "invaid address");
        assert!(err.code() == BuckyErrorCode::AddrInUse);

        let e = BuckyError::bucky_error_to_io_error(err.clone());
        let be = BuckyError::io_error_to_bucky_error(e);
        assert_eq!(be.code(), err.code());
        assert_eq!(be.msg(), err.msg());

        let e: std::io::Error = err.clone().into();
        let be: BuckyError = e.into();

        assert_eq!(be.code(), err.code());
        assert_eq!(be.msg(), err.msg());
    }
}

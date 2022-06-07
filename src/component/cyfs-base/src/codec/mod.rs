mod protobuf;
pub mod raw;
mod serde_codec;
mod format;
mod json_codec;

pub use self::protobuf::*;
pub use raw::*;
pub use serde_codec::*;
pub use json_codec::*;
pub use format::*;

// ObjectContent的编码类型，默认为Raw
pub const OBJECT_CONTENT_CODEC_FORMAT_RAW: u8 = 0;
pub const OBJECT_CONTENT_CODEC_FORMAT_PROTOBUF: u8 = 1;
pub const OBJECT_CONTENT_CODEC_FORMAT_JSON: u8 = 2;


/*
#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq, IntEnum)]
pub enum ObjectContentCodecMethod {
    Unknwon = 255,
    Raw = 0,
    ProtoBuf = 1,
    Json = 2,
}

impl Into<u8> for ObjectContentCodecMethod {
    fn into(self) -> u32 {
        unsafe { std::mem::transmute(self as u8) }
    }
}

impl From<u8> for ObjectContentCodecMethod {
    fn from(code: u8) -> Self {
        match Self::from_int(code) {
            Ok(code) => code,
            Err(e) => {
                error!("unknown codec method: {} {}", code, e);
                Self::Unknown
            }
        }
    }
}
*/

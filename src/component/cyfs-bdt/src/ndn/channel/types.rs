use serde_json::{Map, Value};
use cyfs_base::*;

#[derive(Debug, Clone)]
pub enum PieceSessionType {
    Unknown,
    Stream(u32), 
    RaptorA(u32),  
    RaptorB(u32), 
    // RaptorN(u32)
} 

impl RawEncode for PieceSessionType {
    fn raw_measure(&self, _: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        match self {
            Self::Unknown => Ok(u8::raw_bytes().unwrap()), 
            _ => Ok(u8::raw_bytes().unwrap() + u32::raw_bytes().unwrap())
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            Self::Unknown => 0u8.raw_encode(buf, purpose), 
            Self::Stream(start) => {
                let buf = 1u8.raw_encode(buf, purpose)?;
                start.raw_encode(buf, purpose)
            },
            Self::RaptorA(start) => {
                let buf = 2u8.raw_encode(buf, purpose)?;
                start.raw_encode(buf, purpose)
            },
            Self::RaptorB(start) => {
                let buf = 3u8.raw_encode(buf, purpose)?;
                start.raw_encode(buf, purpose)
            }
        }
    }
}


impl<'de> RawDecode<'de> for PieceSessionType {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (type_code, buf) = u8::raw_decode(buf)?;
        match type_code {
            0u8 => Ok((Self::Unknown, buf)), 
            1u8 => {
                let (start, buf) = u32::raw_decode(buf)?;
                Ok((Self::Stream(start), buf))
            },
            2u8 => {
                let (start, buf) = u32::raw_decode(buf)?;
                Ok((Self::RaptorA(start), buf))
            },
            3u8 => {
                let (start, buf) = u32::raw_decode(buf)?;
                Ok((Self::RaptorB(start), buf))
            },
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidData, "invalid type code"))
        }
    }
}


impl JsonCodec<PieceSessionType> for PieceSessionType {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        match self {
            Self::Unknown => JsonCodecHelper::encode_string_field(&mut obj, "type", "Unknown"), 
            Self::Stream(start) => {
                JsonCodecHelper::encode_string_field(&mut obj, "type", "Stream");
                JsonCodecHelper::encode_option_number_field(&mut obj, "stream_start", Some(*start));
            }, 
            Self::RaptorA(sequence) => {
                JsonCodecHelper::encode_string_field(&mut obj, "type", "RaptorA");
                JsonCodecHelper::encode_option_number_field(&mut obj, "raptor_sequence", Some(*sequence));
            }, 
            Self::RaptorB(sequence) => {
                JsonCodecHelper::encode_string_field(&mut obj, "type", "RaptorB");
                JsonCodecHelper::encode_option_number_field(&mut obj, "raptor_sequence", Some(*sequence));
            } 
        }
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let prefer_type: String = JsonCodecHelper::decode_string_field(obj, "type")?;
        match prefer_type.as_str() {
            "Unknown" => Ok(Self::Unknown), 
            "Stream" => Ok(Self::Stream(JsonCodecHelper::decode_option_int_filed(obj, "stream_start")?
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::InvalidInput, "no stream_start field"))?)), 
            "RaptorA" => Ok(Self::Stream(JsonCodecHelper::decode_option_int_filed(obj, "raptor_sequence")?
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::InvalidInput, "no raptor_sequence field"))?)), 
            "RaptorB" => Ok(Self::Stream(JsonCodecHelper::decode_option_int_filed(obj, "raptor_sequence")?
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::InvalidInput, "no raptor_sequence field"))?)), 
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidInput, format!("invalid type {}", prefer_type)))
        }
    }
}
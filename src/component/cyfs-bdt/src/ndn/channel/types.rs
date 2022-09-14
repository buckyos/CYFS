use serde_json::{Map, Value};
use cyfs_base::*;

const PIECE_SESSION_FLAGS_UNKNOWN: u16 = 0; 
const PIECE_SESSION_FLAGS_STREAM: u16 = 1<<0;
const PIECE_SESSION_FLAGS_RAPTOR: u16 = 1<<1;
const PIECE_SESSION_FLAGS_STREAM_START: u16 = 1<<2; 
const PIECE_SESSION_FLAGS_STREAM_END: u16 = 1<<3; 
const PIECE_SESSION_FLAGS_STREAM_STEP: u16 = 1<<4;
const PIECE_SESSION_FLAGS_RAPTOR_K: u16 = 1<<2; 
const PIECE_SESSION_FLAGS_RAPTOR_SEQ: u16 = 1<<3; 
const PIECE_SESSION_FLAGS_RAPTOR_STEP: u16 = 1<<4;

#[derive(Debug, Clone)]
pub enum PieceSessionType {
    Unknown,
    Stream(Option<u32>, Option<u32>, Option<i32>), 
    Raptor(Option<u32>, Option<u32>, Option<i32>)
} 

impl RawEncode for PieceSessionType {
    fn raw_measure(&self, _: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        match self {
            Self::Unknown => Ok(u16::raw_bytes().unwrap()), 
            Self::Stream(start, end, step) => {
                let mut s = u16::raw_bytes().unwrap();
                s += start.as_ref().map(|_| u32::raw_bytes().unwrap()).unwrap_or_default();
                s += end.as_ref().map(|_| u32::raw_bytes().unwrap()).unwrap_or_default();
                s += step.as_ref().map(|_| i32::raw_bytes().unwrap()).unwrap_or_default();
                Ok(s)
            },
            Self::Raptor(k, seq, step) => {
                let mut s = u16::raw_bytes().unwrap();
                s += k.as_ref().map(|_| u32::raw_bytes().unwrap()).unwrap_or_default();
                s += seq.as_ref().map(|_| u32::raw_bytes().unwrap()).unwrap_or_default();
                s += step.as_ref().map(|_| i32::raw_bytes().unwrap()).unwrap_or_default();
                Ok(s)
            },
        }
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            Self::Unknown => PIECE_SESSION_FLAGS_UNKNOWN.raw_encode(buf, purpose), 
            Self::Stream(start, end, step) => {
                let flags = PIECE_SESSION_FLAGS_STREAM 
                    | start.as_ref().map(|_| PIECE_SESSION_FLAGS_STREAM_START).unwrap_or_default()
                    | end.as_ref().map(|_| PIECE_SESSION_FLAGS_STREAM_END).unwrap_or_default()
                    | step.as_ref().map(|_| PIECE_SESSION_FLAGS_STREAM_STEP).unwrap_or_default();

                let buf = flags.raw_encode(buf, purpose)?;
                let buf = if let Some(start) = start {
                    start.raw_encode(buf, purpose)?
                } else {
                    buf
                };
                let buf = if let Some(end) = end {
                    end.raw_encode(buf, purpose)?
                } else {
                    buf
                };
                
                if let Some(step) = step {
                    step.raw_encode(buf, purpose)
                } else {
                    Ok(buf)
                }
            },
            Self::Raptor(k, seq, step) => {
                let flags = PIECE_SESSION_FLAGS_RAPTOR 
                    | k.as_ref().map(|_| PIECE_SESSION_FLAGS_RAPTOR_K).unwrap_or_default()
                    | seq.as_ref().map(|_| PIECE_SESSION_FLAGS_RAPTOR_SEQ).unwrap_or_default()
                    | step.as_ref().map(|_| PIECE_SESSION_FLAGS_RAPTOR_STEP).unwrap_or_default();

                let buf = flags.raw_encode(buf, purpose)?;
                let buf = if let Some(k) = k {
                    k.raw_encode(buf, purpose)?
                } else {
                    buf
                };
                let buf = if let Some(seq) = seq {
                    seq.raw_encode(buf, purpose)?
                } else {
                    buf
                };
                
                if let Some(step) = step {
                    step.raw_encode(buf, purpose)
                } else {
                    Ok(buf)
                }
            },
        }
    }
}


impl<'de> RawDecode<'de> for PieceSessionType {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (flags, buf) = u16::raw_decode(buf)?;
        if flags == PIECE_SESSION_FLAGS_UNKNOWN {
            Ok((Self::Unknown, buf))
        } else if flags & PIECE_SESSION_FLAGS_STREAM > 0 {
            let (start, buf) = if flags & PIECE_SESSION_FLAGS_STREAM_START > 0 {
                let (start, buf) = u32::raw_decode(buf)?;
                (Some(start), buf)
            } else {
                (None, buf)
            };
            let (end, buf) = if flags & PIECE_SESSION_FLAGS_STREAM_END > 0 {
                let (end, buf) = u32::raw_decode(buf)?;
                (Some(end), buf)
            } else {
                (None, buf)
            };
            let (step, buf) = if flags & PIECE_SESSION_FLAGS_STREAM_STEP > 0 {
                let (step, buf) = i32::raw_decode(buf)?;
                (Some(step), buf)
            } else {
                (None, buf)
            };
            Ok((Self::Stream(start, end, step), buf))
        } else if flags & PIECE_SESSION_FLAGS_RAPTOR > 0 {
            let (k, buf) = if flags & PIECE_SESSION_FLAGS_RAPTOR_K > 0 {
                let (k, buf) = u32::raw_decode(buf)?;
                (Some(k), buf)
            } else {
                (None, buf)
            };
            let (seq, buf) = if flags & PIECE_SESSION_FLAGS_RAPTOR_SEQ > 0 {
                let (seq, buf) = u32::raw_decode(buf)?;
                (Some(seq), buf)
            } else {
                (None, buf)
            };
            let (step, buf) = if flags & PIECE_SESSION_FLAGS_RAPTOR_STEP > 0 {
                let (step, buf) = i32::raw_decode(buf)?;
                (Some(step), buf)
            } else {
                (None, buf)
            };
            Ok((Self::Raptor(k, seq, step), buf))
        } else {
            Err(BuckyError::new(BuckyErrorCode::InvalidData, "invalid flags"))
        }
    }
}


impl JsonCodec<PieceSessionType> for PieceSessionType {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        match self {
            Self::Unknown => JsonCodecHelper::encode_string_field(&mut obj, "type", "Unknown"), 
            Self::Stream(start, end, step) => {
                JsonCodecHelper::encode_string_field(&mut obj, "type", "Stream");
                JsonCodecHelper::encode_option_number_field(&mut obj, "stream_start", start.clone());
                JsonCodecHelper::encode_option_number_field(&mut obj, "stream_end", end.clone());
                JsonCodecHelper::encode_option_number_field(&mut obj, "stream_step", step.clone());
            }, 
            Self::Raptor(k, seq, step) => {
                JsonCodecHelper::encode_string_field(&mut obj, "type", "Raptor");
                JsonCodecHelper::encode_option_number_field(&mut obj, "raptor_k", k.clone());
                JsonCodecHelper::encode_option_number_field(&mut obj, "raptor_seq", seq.clone());
                JsonCodecHelper::encode_option_number_field(&mut obj, "raptor_step", step.clone());
            }, 
        }
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let prefer_type: String = JsonCodecHelper::decode_string_field(obj, "type")?;
        match prefer_type.as_str() {
            "Unknown" => Ok(Self::Unknown), 
            "Stream" => {
                let start = JsonCodecHelper::decode_option_int_filed(obj, "stream_start")?;
                let end = JsonCodecHelper::decode_option_int_filed(obj, "stream_end")?;
                let step = JsonCodecHelper::decode_option_int_filed(obj, "stream_step")?;
                Ok(Self::Stream(start, end, step))
            },
            "Raptor" => {
                let k = JsonCodecHelper::decode_option_int_filed(obj, "raptor_k")?;
                let seq = JsonCodecHelper::decode_option_int_filed(obj, "raptor_seq")?;
                let step = JsonCodecHelper::decode_option_int_filed(obj, "raptor_step")?;
                Ok(Self::Raptor(k, seq, step))
            },
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidInput, format!("invalid type {}", prefer_type)))
        }
    }
}
use std::{
    ops::Range, 
    time::Duration, 
    collections::LinkedList, 
};
use serde::{
    Deserialize,
    Serialize,
};
use serde_json::{Map, Value};
use cyfs_base::*;
use crate::{
    types::*
};
use super::{
    channel::protocol::v0::*
};


#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PieceDesc {
    Raptor(u32 /*raptor seq*/, u16 /*raptor k*/),
    Range(u32 /*range index*/, u16 /*range size*/),
}

impl PieceDesc {
    pub fn raw_raptor_bytes() -> usize {
        u8::raw_bytes().unwrap() + u32::raw_bytes().unwrap() + u16::raw_bytes().unwrap()
    }

    pub fn raw_stream_bytes() -> usize {
        u8::raw_bytes().unwrap() + u32::raw_bytes().unwrap() + u16::raw_bytes().unwrap()
    }

    pub fn unwrap_as_stream(&self) -> (u32, u16) {
        match self {
            Self::Range(index, range) => (*index, *range), 
            Self::Raptor(..) => unreachable!()
        }
    }

    pub fn stream_end_index(chunk: &ChunkId, range: u32) -> u32 {
        (chunk.len() as u32 + range - 1) / range - 1
    }

    pub fn stream_piece_range(&self, chunk: &ChunkId) -> (u32, Range<u64>) {
        match self {
            Self::Range(index, range) => {
                if *index == Self::stream_end_index(chunk, *range as u32) {
                    (*index, (*index * (*range) as u32) as u64..chunk.len() as u64)
                } else {
                    (*index, (*index * (*range) as u32) as u64..((*index + 1) * (*range) as u32) as u64)
                }
            }, 
            Self::Raptor(..) => unreachable!()
        }
    }

    pub fn from_stream_offset(range: usize, offset: u32) -> (Self, u32) {
        let index = offset / range as u32;
        let offset = offset - index * range as u32;
        (Self::Range(index, range as u16), offset)
    }
}

impl RawFixedBytes for PieceDesc {
    fn raw_bytes() -> Option<usize> {
        Some(Self::raw_raptor_bytes())
    }
}

impl RawEncode for PieceDesc {
    fn raw_measure(&self, _purpose: &Option<RawEncodePurpose>) -> BuckyResult<usize> {
        Ok(Self::raw_bytes().unwrap())
    }

    fn raw_encode<'a>(
        &self,
        buf: &'a mut [u8],
        purpose: &Option<RawEncodePurpose>,
    ) -> BuckyResult<&'a mut [u8]> {
        match self {
            Self::Raptor(index, k) => {
                let buf = 0u8.raw_encode(buf, purpose)?;
                let buf = index.raw_encode(buf, purpose)?;
                k.raw_encode(buf, purpose)
            }, 
            Self::Range(index, len) => {
                let buf = 1u8.raw_encode(buf, purpose)?;
                let buf = index.raw_encode(buf, purpose)?;
                len.raw_encode(buf, purpose)
            }
        }
    }
}

impl<'de> RawDecode<'de> for PieceDesc {
    fn raw_decode(buf: &'de [u8]) -> BuckyResult<(Self, &'de [u8])> {
        let (code, buf) = u8::raw_decode(buf)?;
        match code {
            0u8 => {
                let (index, buf) = u32::raw_decode(buf)?;
                let (k, buf) = u16::raw_decode(buf)?;
                Ok((Self::Raptor(index, k), buf))
            }, 
            1u8 => {
                let (index, buf) = u32::raw_decode(buf)?;
                let (len, buf) = u16::raw_decode(buf)?;
                Ok((Self::Range(index, len), buf))
            }, 
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidData, "invalid piece desc type code"))
        }
    }
}


const PIECE_SESSION_FLAGS_UNKNOWN: u16 = 0; 
const PIECE_SESSION_FLAGS_STREAM: u16 = 1<<0;
const PIECE_SESSION_FLAGS_RAPTOR: u16 = 1<<1;
const PIECE_SESSION_FLAGS_STREAM_START: u16 = 1<<2; 
const PIECE_SESSION_FLAGS_STREAM_END: u16 = 1<<3; 
const PIECE_SESSION_FLAGS_STREAM_STEP: u16 = 1<<4;
const PIECE_SESSION_FLAGS_RAPTOR_K: u16 = 1<<2; 
const PIECE_SESSION_FLAGS_RAPTOR_SEQ: u16 = 1<<3; 
const PIECE_SESSION_FLAGS_RAPTOR_STEP: u16 = 1<<4;

#[derive(Debug, Clone, Eq, PartialEq, Serialize)]
pub enum ChunkCodecDesc {
    Unknown,
    Stream(Option<u32>, Option<u32>, Option<i32>), 
    Raptor(Option<u32>, Option<u32>, Option<i32>)
} 

impl ChunkCodecDesc {
    pub fn reverse_stream(start: Option<u32>, end: Option<u32>) -> Self {
        Self::Stream(start, end, Some(-(PieceData::max_payload() as i32)))
    }

    pub fn fill_values(&self, chunk: &ChunkId) -> Self {
        match self {
            Self::Unknown => Self::Unknown, 
            Self::Stream(start, end, step) => {
                let start = start.clone().unwrap_or(0);
                let range = step.map(|s| s.abs() as u32).unwrap_or(PieceData::max_payload() as u32);
                let end = end.clone().unwrap_or(PieceDesc::stream_end_index(chunk, range) + 1);
                let step = step.clone().unwrap_or(range as i32);
                Self::Stream(Some(start), Some(end), Some(step))
            }, 
            Self::Raptor(..) => unimplemented!()
        }
    }

    pub fn unwrap_as_stream(&self) -> (u32, u32, i32) {
        match self {
            Self::Stream(start, end, step) => ((*start).unwrap(), (*end).unwrap(), (*step).unwrap()), 
            _ => unreachable!()
        }
    }

    pub fn support_desc(&self, other: &Self) -> bool {
        match self {
            Self::Unknown => true, 
            Self::Stream(self_start, self_end, self_step) => {
                match other {
                    Self::Unknown => true,
                    Self::Stream(..) => {
                        let (other_start, other_end, other_step) = other.unwrap_as_stream();
                        if let Some(self_step) = self_step {
                            if *self_step * other_step < 0 {
                                return false
                            }

                            if other_step.abs() > self_step.abs() {
                                return false;
                            }
                        }

                        if let Some(self_start) = self_start {
                            if *self_start > other_start {
                                return false;
                            }
                        }

                        if let Some(self_end) = self_end {
                            if *self_end < other_end {
                                return false;
                            }
                        }

                        true
                    }, 
                    Self::Raptor(..) => false
                }
            }, 
            Self::Raptor(..) => unimplemented!()
        }

    }
}

impl RawEncode for ChunkCodecDesc {
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


impl<'de> RawDecode<'de> for ChunkCodecDesc {
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


impl JsonCodec<ChunkCodecDesc> for ChunkCodecDesc {
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
                let start = JsonCodecHelper::decode_option_int_field(obj, "stream_start")?;
                let end = JsonCodecHelper::decode_option_int_field(obj, "stream_end")?;
                let step = JsonCodecHelper::decode_option_int_field(obj, "stream_step")?;
                Ok(Self::Stream(start, end, step))
            },
            "Raptor" => {
                let k = JsonCodecHelper::decode_option_int_field(obj, "raptor_k")?;
                let seq = JsonCodecHelper::decode_option_int_field(obj, "raptor_seq")?;
                let step = JsonCodecHelper::decode_option_int_field(obj, "raptor_step")?;
                Ok(Self::Raptor(k, seq, step))
            },
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidInput, format!("invalid type {}", prefer_type)))
        }
    }
}



#[derive(Clone)]
pub struct HistorySpeedConfig {
    pub attenuation: f64, 
    pub atomic: Duration, 
    pub expire: Duration
}

#[derive(Clone)]
// 计算历史速度的方法， 在过去的一段时间内，  Sum(speed(t)*(衰减^t))/样本数
pub struct HistorySpeed {
    expire_count: usize, 
    config: HistorySpeedConfig, 
    intermediate: LinkedList<f64>, 
    last_update: Timestamp
}

impl HistorySpeed {
    pub fn new(initial: u32, config: HistorySpeedConfig) -> Self {
        let mut intermediate = LinkedList::new();
        intermediate.push_back(initial as f64);

        Self {
            expire_count: (config.expire.as_micros() / config.atomic.as_micros()) as usize, 
            config, 
            intermediate, 
            last_update: bucky_time_now() 
        }   
    }

    pub fn update(&mut self, cur_speed: Option<u32>, when: Timestamp) {
        let cur_speed = cur_speed.unwrap_or(self.latest());

        if when > self.last_update {
            let mut count = ((when - self.last_update) / self.config.atomic.as_micros() as u64) as usize;

            if count > self.expire_count {
                self.intermediate.clear();
                count = self.expire_count;
            }

            for _ in 0..count {
                self.intermediate.iter_mut().for_each(|v| *v = (*v) * self.config.attenuation);
                self.intermediate.push_back(cur_speed as f64);
                if self.intermediate.len() > self.expire_count {
                    self.intermediate.pop_front();
                }
            }

            self.last_update = when;
        };
    }

    pub fn average(&self) -> u32 {
        let total: f64 = self.intermediate.iter().sum();
        (total / self.intermediate.len() as f64) as u32
    }

    pub fn latest(&self) -> u32 {
        self.intermediate.back().cloned().unwrap() as u32
    }

    pub fn config(&self) -> &HistorySpeedConfig {
        &self.config
    }
}


pub struct SpeedCounter {
    last_recv: u64, 
    last_update: Timestamp, 
    cur_speed: u32
}


impl SpeedCounter {
    pub fn new(init_recv: usize) -> Self {
        Self {
            last_recv: init_recv as u64, 
            last_update: bucky_time_now(), 
            cur_speed: 0
        }
    }

    pub fn on_recv(&mut self, recv: usize) {
        self.last_recv += recv as u64;
    }

    pub fn update(&mut self, when: Timestamp) -> u32 {
        if when > self.last_update {
            let last_recv = self.last_recv;
            self.last_recv = 0;
            self.cur_speed = ((last_recv * 1000 * 1000) as f64 / (when - self.last_update) as f64) as u32;
            self.cur_speed
        } else {
            self.cur_speed
        }
    }

    pub fn cur(&self) -> u32 {
        self.cur_speed
    }
}



// 对scheduler的接口
#[derive(Debug, Serialize, Deserialize)]
pub enum NdnTaskState {
    Running(u32/*速度*/),
    Paused,
    Error(BuckyError/*被cancel的原因*/), 
    Finished
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum NdnTaskControlState {
    Normal, 
    Paused, 
    Canceled, 
}

pub trait NdnTask: Send + Sync {
    fn clone_as_task(&self) -> Box<dyn NdnTask>;
    fn state(&self) -> NdnTaskState;
    fn control_state(&self) -> NdnTaskControlState;

    fn resume(&self) -> BuckyResult<NdnTaskControlState> {
        Ok(NdnTaskControlState::Normal)
    }
    fn cancel(&self) -> BuckyResult<NdnTaskControlState> {
        Ok(NdnTaskControlState::Normal)
    }
    fn pause(&self) -> BuckyResult<NdnTaskControlState> {
        Ok(NdnTaskControlState::Normal)
    }
    
    fn close(&self) -> BuckyResult<()> {
        Ok(())
    }

    fn cur_speed(&self) -> u32;
    fn history_speed(&self) -> u32;
}
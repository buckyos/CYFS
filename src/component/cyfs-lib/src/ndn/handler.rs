use serde_json::{Map, Value};
use cyfs_base::*;
use cyfs_bdt::{
    TempSeq, 
    ndn::channel::{ChunkEncodeDesc}
};
use super::{
    bdt_request::BdtDataRefererInfo
};

pub struct InterestHandlerRequest {
    pub session_id: TempSeq, 
    pub chunk: ChunkId,
    pub prefer_type: ChunkEncodeDesc, 
    pub from: Option<DeviceId>,
    pub referer: Option<BdtDataRefererInfo>, 
    pub from_channel: DeviceId 
}


impl JsonCodec<InterestHandlerRequest> for InterestHandlerRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_number_field(&mut obj, "session_id", self.session_id.value());
        JsonCodecHelper::encode_string_field(&mut obj, "chunk", &self.chunk);
        JsonCodecHelper::encode_field(&mut obj, "prefer_type", &self.prefer_type);
        JsonCodecHelper::encode_option_string_field(&mut obj, "from", self.from.as_ref());
        JsonCodecHelper::encode_option_field(&mut obj, "referer", self.referer.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "from_channel", &self.from_channel);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let session_id: u32 = JsonCodecHelper::decode_int_field(obj, "session_id")?;
        Ok(Self {
            session_id: TempSeq::from(session_id), 
            chunk: JsonCodecHelper::decode_string_field(obj, "chunk")?, 
            prefer_type: JsonCodecHelper::decode_field(obj, "prefer_type")?, 
            from: JsonCodecHelper::decode_option_string_field(obj, "from")?, 
            referer: JsonCodecHelper::decode_option_field(obj, "referer")?, 
            from_channel: JsonCodecHelper::decode_string_field(obj, "from_channel")?, 
        })
    }
}


impl std::fmt::Display for InterestHandlerRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "session_id: {:?}", self.session_id)?;
        write!(f, ", chunk: {}", self.chunk)?;
        write!(f, ", prefer_type: {:?}", self.prefer_type)?;
        if let Some(from) = &self.from {
            write!(f, ", from: {}", from)?;
        }
        if let Some(referer) = &self.referer {
            write!(f, ", referer: {}", referer)?;
        }
        
        write!(f, ", from_channel: {:?}", self.from_channel)
    }
}


#[derive(Clone)]
pub struct RespInterestFields {
    pub err: BuckyErrorCode,
    pub redirect: Option<DeviceId>,
    pub redirect_referer_target: Option<ObjectId>,
}

impl std::fmt::Display for RespInterestFields {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "err: {}", self.err)?;
        if let Some(redierct) = &self.redirect {
            write!(f, ", redierct: {}", redierct)?;
        }
        if let Some(obj_id) = &self.redirect_referer_target {
            write!(f, ", redirect_referer_target: {}", obj_id)?;
        }
        Ok(())
    }
}


impl JsonCodec<RespInterestFields> for RespInterestFields {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        let err: u32 = self.err.into();
        JsonCodecHelper::encode_number_field(&mut obj, "err", err);
        JsonCodecHelper::encode_option_string_field(&mut obj, "redirect", self.redirect.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "redirect_referer_target", self.redirect_referer_target.as_ref());
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let err: u32 = JsonCodecHelper::decode_int_field(obj, "err")?;
        Ok(Self {
            err: BuckyErrorCode::from(err), 
            redirect: JsonCodecHelper::decode_option_string_field(obj, "redirect")?, 
            redirect_referer_target: JsonCodecHelper::decode_option_string_field(obj, "decode_option_string_field")?, 
        })
    }
}

#[derive(Clone)]
pub enum InterestHandlerResponse {
    Default, 
    Upload(Vec<String>), 
    Transmit(DeviceId), 
    Resp(RespInterestFields), 
    Handled
}

impl InterestHandlerResponse {
    pub fn type_str(&self) -> &str {
        match self {
            Self::Default => "Default", 
            Self::Upload(_) => "Upload",
            Self::Transmit(_) => "Transmit",  
            Self::Resp(_) => "Resp", 
            Self::Handled => "Handled"
        }
    }

    pub fn resp_interest(&self) -> Option<&RespInterestFields> {
        if let Self::Resp(resp) = self {
            Some(resp)
        } else {
            None
        }
    }

    pub fn transmit_to(&self) -> Option<&DeviceId> {
        if let Self::Transmit(to) = self {
            Some(to)
        } else {
            None
        }
    }
}

impl JsonCodec<InterestHandlerResponse> for InterestHandlerResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        match self {
            Self::Default => JsonCodecHelper::encode_string_field(&mut obj, "type", "Default"), 
            Self::Upload(groups) => {
                JsonCodecHelper::encode_string_field(&mut obj, "type", "Upload");
                JsonCodecHelper::encode_str_array_field(&mut obj, "upload_groups", groups);
            }, 
            Self::Transmit(to) => {
                JsonCodecHelper::encode_string_field(&mut obj, "type", "Transmit");
                JsonCodecHelper::encode_option_string_field(&mut obj, "transmit_to", Some(to));
            }, 
            Self::Resp(resp) => {
                JsonCodecHelper::encode_string_field(&mut obj, "type", "Resp");
                JsonCodecHelper::encode_option_field(&mut obj, "resp", Some(resp));
            }, 
            Self::Handled => JsonCodecHelper::encode_string_field(&mut obj, "type", "Handled") 
        }
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let type_str: String = JsonCodecHelper::decode_string_field(obj, "type")?;
        match type_str.as_str() {
            "Default" => Ok(Self::Default), 
            "Upload" => {
                let groups = JsonCodecHelper::decode_str_array_field(obj, "upload_groups")?;
                Ok(Self::Upload(groups))
            }, 
            "Transmit" => Ok(Self::Transmit(JsonCodecHelper::decode_option_string_field(obj, "transmit_to")?
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::InvalidInput, "no transmit_to field"))?)), 
            "Resp" => Ok(Self::Resp(JsonCodecHelper::decode_option_field(obj, "resp")?
                .ok_or_else(|| BuckyError::new(BuckyErrorCode::InvalidInput, "no resp field"))?)), 
            "Handled" => Ok(Self::Handled), 
            _ => Err(BuckyError::new(BuckyErrorCode::InvalidInput, format!("invalid type {}", type_str)))
        }
    }
}


impl std::fmt::Display for InterestHandlerResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Default => write!(f, "Default")?, 
            Self::Upload(groups) => write!(f, "Upload({:?})", groups)?, 
            Self::Transmit(to) => write!(f, "Transmit({})", to)?, 
            Self::Resp(resp) => write!(f, "Resp({})", resp)?, 
            Self::Handled => write!(f, "Handled")?
        }
        Ok(())
    }
}


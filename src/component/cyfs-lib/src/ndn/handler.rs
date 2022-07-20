use serde_json::{Map, Value};
use cyfs_base::*;
use cyfs_bdt::{
    ndn::channel::protocol::{Interest, RespInterest}
};

#[derive(Debug, Clone)]
pub struct InterestHandlerRequest {
    pub interest: Interest, 
    pub from_channel: DeviceId 
}


impl JsonCodec<InterestHandlerRequest> for InterestHandlerRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "interest", &self.interest);
        JsonCodecHelper::encode_string_field(&mut obj, "from_channel", &self.from_channel);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            interest: JsonCodecHelper::decode_field(obj, "interest")?, 
            from_channel: JsonCodecHelper::decode_string_field(obj, "from_channel")?, 
        })
    }
}


impl std::fmt::Display for InterestHandlerRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "interest: {:?}", self.interest)?;
        write!(f, ", from_channel: {:?}", self.from_channel)
    }
}


#[derive(Debug, Clone)]
pub enum InterestHandlerResponse {
    Upload, 
    Resp(RespInterest), 
    Handled
}

impl InterestHandlerResponse {
    pub fn type_str(&self) -> &str {
        match self {
            Self::Upload => "Upload", 
            Self::Resp(_) => "Resp", 
            Self::Handled => "Handled"
        }
    }

    pub fn resp_interest(&self) -> Option<&RespInterest> {
        if let Self::Resp(resp) = self {
            Some(resp)
        } else {
            None
        }
    }
}

impl JsonCodec<InterestHandlerResponse> for InterestHandlerResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        match self {
            Self::Upload =>  JsonCodecHelper::encode_string_field(&mut obj, "type", "Upload"), 
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
            "Upload" => Ok(Self::Upload), 
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
            Self::Upload => write!(f, "Upload")?, 
            Self::Resp(resp) => write!(f, "Resp({:?})", resp)?, 
            Self::Handled => write!(f, "Handled")?
        }
        Ok(())
    }
}
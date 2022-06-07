use super::super::AclAction;
use super::request::*;
use cyfs_base::*;

use serde_json::{Map, Value};

impl JsonCodec<AclAction> for AclAction {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "direction", &self.direction);
        JsonCodecHelper::encode_string_field(&mut obj, "operation", &self.operation);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            direction: JsonCodecHelper::decode_string_field(obj, "direction")?,
            operation: JsonCodecHelper::decode_string_field(obj, "operation")?,
        })
    }
}

impl JsonCodec<AclHandlerRequest> for AclHandlerRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "protocol", &self.protocol);
        JsonCodecHelper::encode_field(&mut obj, "action", &self.action);

        JsonCodecHelper::encode_string_field(&mut obj, "device_id", &self.device_id);

        JsonCodecHelper::encode_option_field(&mut obj, "object", self.object.as_ref());
        
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "inner_path",
            self.inner_path.as_ref(),
        );

        JsonCodecHelper::encode_option_string_field(&mut obj, "req_path", self.req_path.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "dec_id", &self.dec_id);
        JsonCodecHelper::encode_option_str_array_field(&mut obj, "referer_object", self.referer_object.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            protocol: JsonCodecHelper::decode_string_field(obj, "protocol")?,
            action: JsonCodecHelper::decode_field(obj, "action")?,
            device_id: JsonCodecHelper::decode_string_field(obj, "device_id")?,

            object: JsonCodecHelper::decode_option_field(obj, "object")?,
            inner_path: JsonCodecHelper::decode_option_string_field(obj, "inner_path")?,

            req_path: JsonCodecHelper::decode_option_string_field(obj, "req_path")?,
            dec_id: JsonCodecHelper::decode_string_field(obj, "dec_id")?,
            referer_object: JsonCodecHelper::decode_option_str_array_field(obj, "referer_object")?,
        })
    }
}


impl JsonCodec<AclHandlerResponse> for AclHandlerResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "access", &self.access);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            access: JsonCodecHelper::decode_string_field(obj, "access")?,
        })
    }
}
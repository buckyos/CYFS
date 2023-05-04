use super::request::*;
use cyfs_base::*;

use serde_json::{Map, Value};


impl JsonCodec<AclHandlerRequest> for AclHandlerRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "dec_id", &self.dec_id);
        JsonCodecHelper::encode_field(&mut obj, "source", &self.source);

        JsonCodecHelper::encode_string_field(&mut obj, "req_path", &self.req_path);
        JsonCodecHelper::encode_option_string_field(&mut obj, "req_query_string", self.req_query_string.as_ref());

        JsonCodecHelper::encode_string_field(&mut obj, "permissions", &self.permissions);
        
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            dec_id: JsonCodecHelper::decode_string_field(obj, "dec_id")?,
            source: JsonCodecHelper::decode_field(obj, "source")?,

            req_path: JsonCodecHelper::decode_string_field(obj, "req_path")?,
            req_query_string: JsonCodecHelper::decode_option_string_field(obj, "req_query_string")?,

            permissions: JsonCodecHelper::decode_string_field(obj, "permissions")?,
        })
    }
}


impl JsonCodec<AclHandlerResponse> for AclHandlerResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "action", &self.action);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            action: JsonCodecHelper::decode_string_field(obj, "action")?,
        })
    }
}
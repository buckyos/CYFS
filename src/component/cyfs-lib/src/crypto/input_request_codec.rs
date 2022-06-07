use super::input_request::*;
use cyfs_base::*;

use serde_json::{Map, Value};

impl JsonCodec<CryptoInputRequestCommon> for CryptoInputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "req_path", self.req_path.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_id", self.dec_id.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "source", &self.source);
        JsonCodecHelper::encode_string_field(&mut obj, "protocol", &self.protocol);
        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<CryptoInputRequestCommon> {
        Ok(Self {
            req_path: JsonCodecHelper::decode_option_string_field(obj, "req_path")?,
            dec_id: JsonCodecHelper::decode_option_string_field(obj, "dec_id")?,

            source: JsonCodecHelper::decode_string_field(obj, "source")?,
            protocol: JsonCodecHelper::decode_string_field(obj, "protocol")?,

            target: JsonCodecHelper::decode_option_string_field(obj, "target")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

// sign_object
impl JsonCodec<CryptoSignObjectInputRequest> for CryptoSignObjectInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_field(&mut obj, "object", &self.object);
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<CryptoSignObjectInputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object: JsonCodecHelper::decode_field(obj, "object")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

impl JsonCodec<CryptoSignObjectInputResponse> for CryptoSignObjectInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "result", &self.result);
        JsonCodecHelper::encode_option_field(&mut obj, "object", self.object.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<CryptoSignObjectInputResponse> {
        Ok(Self {
            result: JsonCodecHelper::decode_string_field(obj, "result")?,
            object: JsonCodecHelper::decode_option_field(obj, "object")?,
        })
    }
}

// verify_object
impl JsonCodec<CryptoVerifyObjectInputRequest> for CryptoVerifyObjectInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        JsonCodecHelper::encode_string_field(&mut obj, "sign_type", &self.sign_type);

        JsonCodecHelper::encode_field(&mut obj, "object", &self.object);

        JsonCodecHelper::encode_field(&mut obj, "sign_object", &self.sign_object);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<CryptoVerifyObjectInputRequest> {
       
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            sign_type: JsonCodecHelper::decode_string_field(obj, "sign_type")?,
            object: JsonCodecHelper::decode_field(obj, "object")?,
            sign_object: JsonCodecHelper::decode_field(obj, "sign_object")?,
        })
    }
}
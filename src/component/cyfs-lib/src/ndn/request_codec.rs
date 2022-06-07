use super::output_request::*;
use cyfs_base::*;

use serde_json::{Map, Value};

impl JsonCodec<NDNOutputRequestCommon> for NDNOutputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "req_path", self.req_path.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_id", self.dec_id.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "level", &self.level);
        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_str_array_field(&mut obj, "referer_object", &self.referer_object);
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            req_path: JsonCodecHelper::decode_option_string_field(obj, "req_path")?,
            dec_id: JsonCodecHelper::decode_option_string_field(obj, "dec_id")?,
            level: JsonCodecHelper::decode_string_field(obj, "level")?,
            target: JsonCodecHelper::decode_option_string_field(obj, "target")?,
            referer_object: JsonCodecHelper::decode_str_array_field(obj, "referer_object")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

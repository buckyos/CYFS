use cyfs_base::{BuckyResult, JsonCodec, JsonCodecAutoWithSerde, JsonCodecHelper};
use serde_json::{Map, Value};

use crate::{output_request::GroupStartServiceOutputRequest, GroupStartServiceInputRequest};

impl JsonCodec<GroupStartServiceOutputRequest> for GroupStartServiceOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "group-id", &self.group_id);
        JsonCodecHelper::encode_string_field(&mut obj, "rpath", self.rpath.as_str());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            group_id: JsonCodecHelper::decode_string_field(obj, "group-id")?,
            rpath: JsonCodecHelper::decode_string_field(obj, "rpath")?,
        })
    }
}

impl JsonCodec<GroupStartServiceInputRequest> for GroupStartServiceInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "group-id", &self.group_id);
        JsonCodecHelper::encode_string_field(&mut obj, "rpath", self.rpath.as_str());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            group_id: JsonCodecHelper::decode_string_field(obj, "group-id")?,
            rpath: JsonCodecHelper::decode_string_field(obj, "rpath")?,
        })
    }
}

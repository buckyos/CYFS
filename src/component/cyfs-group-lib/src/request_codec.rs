use cyfs_base::{BuckyResult, JsonCodec, JsonCodecHelper};
use serde_json::{Map, Value};

use crate::{
    output_request::GroupStartServiceOutputRequest, GroupInputRequestCommon,
    GroupOutputRequestCommon, GroupStartServiceInputRequest,
};

impl JsonCodec<GroupOutputRequestCommon> for GroupOutputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_option_string_field(&mut obj, "dec-id", self.dec_id.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            dec_id: JsonCodecHelper::decode_option_string_field(obj, "dec-id")?,
        })
    }
}

impl JsonCodec<GroupInputRequestCommon> for GroupInputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_field(&mut obj, "source", &self.source);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            source: JsonCodecHelper::decode_field(obj, "source")?,
        })
    }
}

impl JsonCodec<GroupStartServiceOutputRequest> for GroupStartServiceOutputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "group-id", &self.group_id);
        JsonCodecHelper::encode_string_field(&mut obj, "rpath", self.rpath.as_str());
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            group_id: JsonCodecHelper::decode_string_field(obj, "group-id")?,
            rpath: JsonCodecHelper::decode_string_field(obj, "rpath")?,
            common: JsonCodecHelper::decode_field(obj, "common")?,
        })
    }
}

impl JsonCodec<GroupStartServiceInputRequest> for GroupStartServiceInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();
        JsonCodecHelper::encode_string_field(&mut obj, "group-id", &self.group_id);
        JsonCodecHelper::encode_string_field(&mut obj, "rpath", self.rpath.as_str());
        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            group_id: JsonCodecHelper::decode_string_field(obj, "group-id")?,
            rpath: JsonCodecHelper::decode_string_field(obj, "rpath")?,
            common: JsonCodecHelper::decode_field(obj, "common")?,
        })
    }
}

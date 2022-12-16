use super::input_request::*;
use crate::base::NDNDataRequestRange;
use cyfs_base::*;

use serde_json::{Map, Value};

impl JsonCodec<NDNInputRequestCommon> for NDNInputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "req_path", self.req_path.as_ref());
        JsonCodecHelper::encode_field(&mut obj, "source", &self.source);
        JsonCodecHelper::encode_string_field(&mut obj, "level", &self.level);
        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_str_array_field(&mut obj, "referer_object", &self.referer_object);
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            req_path: JsonCodecHelper::decode_option_string_field(obj, "req_path")?,
            source: JsonCodecHelper::decode_field(obj, "source")?,
            level: JsonCodecHelper::decode_string_field(obj, "level")?,
            referer_object: JsonCodecHelper::decode_str_array_field(obj, "referer_object")?,
            target: JsonCodecHelper::decode_option_string_field(obj, "target")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
            user_data: None,
        })
    }
}

// get_data
impl JsonCodec<NDNGetDataInputRequest> for NDNGetDataInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);
        JsonCodecHelper::encode_string_field(&mut obj, "data_type", &self.data_type);

        if let Some(range) = &self.range {
            JsonCodecHelper::encode_string_field_2(&mut obj, "range", range.encode_string());
        }

        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "inner_path",
            self.inner_path.as_ref(),
        );

        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "group",
            self.group.as_ref(),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let range = JsonCodecHelper::decode_option_string_field(obj, "range")?
            .map(|s: String| NDNDataRequestRange::new_unparsed(s));

        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            data_type: JsonCodecHelper::decode_string_field(obj, "data_type")?,
            range,
            inner_path: JsonCodecHelper::decode_option_string_field(obj, "inner_path")?,
            group: JsonCodecHelper::decode_option_string_field(obj, "group")?,
        })
    }
}

impl JsonCodec<NDNGetDataInputResponse> for NDNGetDataInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);
        JsonCodecHelper::encode_option_string_field(&mut obj, "owner_id", self.owner_id.as_ref());
        JsonCodecHelper::encode_option_number_field(
            &mut obj,
            "attr",
            self.attr.as_ref().map(|v| v.flags()),
        );
        JsonCodecHelper::encode_option_string_field(&mut obj, "group", self.group.as_ref());
        JsonCodecHelper::encode_option_field(&mut obj, "range", self.range.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "length", &self.length);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let attr =
            JsonCodecHelper::decode_option_int_field(obj, "attr")?.map(|v| Attributes::new(v));

        // 现在事件不支持data的返回和修改
        let data = Box::new(async_std::io::Cursor::new(vec![]));

        Ok(Self {
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            owner_id: JsonCodecHelper::decode_option_string_field(obj, "owner_id")?,
            attr,
            range: JsonCodecHelper::decode_option_field(obj, "range")?,
            group: JsonCodecHelper::decode_option_string_field(obj, "group")?,
            length: JsonCodecHelper::decode_int_field(obj, "length")?,
            data,
        })
    }
}

// put_data
impl JsonCodec<NDNPutDataInputRequest> for NDNPutDataInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);
        JsonCodecHelper::encode_string_field(&mut obj, "data_type", &self.data_type);
        JsonCodecHelper::encode_string_field(&mut obj, "length", &self.length);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        // 现在事件不支持data的返回和修改
        let data = Box::new(async_std::io::Cursor::new(vec![]));
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            data_type: JsonCodecHelper::decode_string_field(obj, "data_type")?,
            length: JsonCodecHelper::decode_int_field(obj, "length")?,
            data,
        })
    }
}

impl JsonCodec<NDNPutDataInputResponse> for NDNPutDataInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "result", &self.result);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            result: JsonCodecHelper::decode_string_field(obj, "result")?,
        })
    }
}

// delete_data
impl JsonCodec<NDNDeleteDataInputRequest> for NDNDeleteDataInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "inner_path",
            self.inner_path.as_ref(),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            inner_path: JsonCodecHelper::decode_option_string_field(obj, "inner_path")?,
        })
    }
}

impl JsonCodec<NDNDeleteDataInputResponse> for NDNDeleteDataInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
        })
    }
}

// query file
impl JsonCodec<Self> for NDNQueryFileParam {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        let (t, v) = self.to_key_pair();
        JsonCodecHelper::encode_string_field(&mut obj, "type", t);
        JsonCodecHelper::encode_string_field(&mut obj, "value", &v);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let t: String = JsonCodecHelper::decode_string_field(obj, "type")?;
        let value: String = JsonCodecHelper::decode_string_field(obj, "value")?;

        Self::from_key_pair(&t, &value)
    }
}

impl JsonCodec<Self> for NDNQueryFileInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "file_id", &self.file_id);
        JsonCodecHelper::encode_string_field(&mut obj, "hash", &self.hash);
        JsonCodecHelper::encode_string_field(&mut obj, "length", &self.length);
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);
        JsonCodecHelper::encode_option_string_field(&mut obj, "owner", self.owner.as_ref());
        JsonCodecHelper::encode_option_str_array_field(
            &mut obj,
            "quick_hash",
            self.quick_hash.as_ref(),
        );
        JsonCodecHelper::encode_as_option_list(&mut obj, "ref_dirs", self.ref_dirs.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            file_id: JsonCodecHelper::decode_string_field(obj, "file_id")?,
            hash: JsonCodecHelper::decode_string_field(obj, "hash")?,
            length: JsonCodecHelper::decode_string_field(obj, "length")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,

            owner: JsonCodecHelper::decode_option_string_field(obj, "owner")?,
            quick_hash: JsonCodecHelper::decode_option_str_array_field(obj, "quick_hash")?,
            ref_dirs: JsonCodecHelper::decode_option_array_field(obj, "ref_dirs")?,
        })
    }
}

impl JsonCodec<Self> for NDNQueryFileInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_as_list(&mut obj, "list", &self.list);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            list: JsonCodecHelper::decode_array_field(obj, "list")?,
        })
    }
}

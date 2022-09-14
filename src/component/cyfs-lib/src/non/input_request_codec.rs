use super::input_request::*;
use cyfs_base::*;

use serde_json::{Map, Value};


impl JsonCodec<NONInputRequestCommon> for NONInputRequestCommon {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_string_field(&mut obj, "req_path", self.req_path.as_ref());
        JsonCodecHelper::encode_field(&mut obj, "source", &self.source);
        JsonCodecHelper::encode_string_field(&mut obj, "level", &self.level);
        JsonCodecHelper::encode_option_string_field(&mut obj, "target", self.target.as_ref());
        JsonCodecHelper::encode_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONInputRequestCommon> {
        Ok(Self {
            req_path: JsonCodecHelper::decode_option_string_field(obj, "req_path")?,
            source: JsonCodecHelper::decode_field(obj, "source")?,
            level: JsonCodecHelper::decode_string_field(obj, "level")?,
            target: JsonCodecHelper::decode_option_string_field(obj, "target")?,
            flags: JsonCodecHelper::decode_int_field(obj, "flags")?,
        })
    }
}

// get_object
impl JsonCodec<NONGetObjectInputRequest> for NONGetObjectInputRequest {
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

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONGetObjectInputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            inner_path: JsonCodecHelper::decode_option_string_field(obj, "inner_path")?,
        })
    }
}

impl JsonCodec<NONGetObjectInputResponse> for NONGetObjectInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "object", &self.object);
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "object_expires_time",
            self.object_expires_time.as_ref(),
        );
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "object_update_time",
            self.object_update_time.as_ref(),
        );

        JsonCodecHelper::encode_option_number_field(
            &mut obj,
            "attr",
            self.attr.as_ref().map(|v| v.flags()),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONGetObjectInputResponse> {
        let attr = JsonCodecHelper::decode_option_int_filed(obj, "attr")?;

        Ok(Self {
            object: JsonCodecHelper::decode_field(obj, "object")?,
            object_expires_time: JsonCodecHelper::decode_option_string_field(
                obj,
                "object_expires_time",
            )?,
            object_update_time: JsonCodecHelper::decode_option_string_field(
                obj,
                "object_update_time",
            )?,
            attr: attr.map(|v| Attributes::new(v)),
        })
    }
}

// put_object
impl JsonCodec<NONPutObjectInputRequest> for NONPutObjectInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        JsonCodecHelper::encode_field(&mut obj, "object", &self.object);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONPutObjectInputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object: JsonCodecHelper::decode_field(obj, "object")?,
        })
    }
}

impl JsonCodec<NONPutObjectInputResponse> for NONPutObjectInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "result", &self.result);
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "object_expires_time",
            self.object_expires_time.as_ref(),
        );
        JsonCodecHelper::encode_option_string_field(
            &mut obj,
            "object_update_time",
            self.object_update_time.as_ref(),
        );

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONPutObjectInputResponse> {
        Ok(Self {
            result: JsonCodecHelper::decode_string_field(obj, "result")?,
            object_expires_time: JsonCodecHelper::decode_option_string_field(
                obj,
                "object_expires_time",
            )?,
            object_update_time: JsonCodecHelper::decode_option_string_field(
                obj,
                "object_update_time",
            )?,
        })
    }
}

// post_object
impl JsonCodec<NONPostObjectInputRequest> for NONPostObjectInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_field(&mut obj, "object", &self.object);
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONPostObjectInputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object: JsonCodecHelper::decode_field(obj, "object")?,
        })
    }
}

impl JsonCodec<NONPostObjectInputResponse> for NONPostObjectInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_field(&mut obj, "object", self.object.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONPostObjectInputResponse> {
        Ok(Self {
            object: JsonCodecHelper::decode_option_field(obj, "object")?,
        })
    }
}

// select_object
impl JsonCodec<NONSelectObjectInputRequest> for NONSelectObjectInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);

        JsonCodecHelper::encode_field(&mut obj, "filter", &self.filter);
        JsonCodecHelper::encode_option_field(&mut obj, "opt", self.opt.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONSelectObjectInputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            filter: JsonCodecHelper::decode_field(obj, "filter")?,
            opt: JsonCodecHelper::decode_option_field(obj, "opt")?,
        })
    }
}

impl JsonCodec<NONSelectObjectInputResponse> for NONSelectObjectInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_as_list(&mut obj, "objects", &self.objects);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONSelectObjectInputResponse> {
        Ok(Self {
            objects: JsonCodecHelper::decode_array_field(obj, "objects")?,
        })
    }
}

// delete_object
impl JsonCodec<NONDeleteObjectInputRequest> for NONDeleteObjectInputRequest {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "common", &self.common);
        JsonCodecHelper::encode_string_field(&mut obj, "object_id", &self.object_id);
        JsonCodecHelper::encode_option_string_field(&mut obj, "inner_path", self.inner_path.as_ref());

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONDeleteObjectInputRequest> {
        Ok(Self {
            common: JsonCodecHelper::decode_field(obj, "common")?,
            object_id: JsonCodecHelper::decode_string_field(obj, "object_id")?,
            inner_path:  JsonCodecHelper::decode_option_string_field(obj, "inner_path")?,
        })
    }
}

impl JsonCodec<NONDeleteObjectInputResponse> for NONDeleteObjectInputResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        if let Some(object) = &self.object {
            JsonCodecHelper::encode_field(&mut obj, "object", object);
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<NONDeleteObjectInputResponse> {
        Ok(Self { 
            object: JsonCodecHelper::decode_option_field(obj, "object")?
         })
    }
}

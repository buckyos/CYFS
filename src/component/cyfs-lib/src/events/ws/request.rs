use super::super::*;
use crate::router_handler::*;
use cyfs_base::*;

use serde_json::{Map, Value};



#[derive(Debug)]
pub struct RouterWSAddEventParam {
    pub category: RouterEventCategory,
    pub id: String,
    pub index: i32,
    pub routine: String,
}

pub struct RouterWSRemoveEventParam {
    pub category: RouterEventCategory,
    pub id: String,
}

impl JsonCodec<Self> for RouterWSAddEventParam {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "category", &self.category);
        JsonCodecHelper::encode_string_field(&mut obj, "id", &self.id);
        JsonCodecHelper::encode_string_field(&mut obj, "index", &self.index);
        JsonCodecHelper::encode_string_field(&mut obj, "routine", &self.routine);

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            category: JsonCodecHelper::decode_string_field(req_obj, "category")?,
            id: JsonCodecHelper::decode_string_field(req_obj, "id")?,
            index: JsonCodecHelper::decode_string_field(req_obj, "index")?,
            routine: JsonCodecHelper::decode_string_field(req_obj, "routine")?,
        })
    }
}

impl JsonCodec<Self> for RouterWSRemoveEventParam {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "category", &self.category);
        JsonCodecHelper::encode_string_field(&mut obj, "id", &self.id);

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            category: JsonCodecHelper::decode_string_field(req_obj, "category")?,
            id: JsonCodecHelper::decode_string_field(req_obj, "id")?,
        })
    }
}

pub type RouterWSEventResponse = RouterWSHandlerResponse;

pub struct RouterWSEventEmitParam {
    pub category: RouterEventCategory,

    pub id: String,

    pub param: String,
}

impl RouterWSEventEmitParam {
    pub fn encode_json_impl<P>(
        category: &RouterEventCategory,
        id: &str,
        param: &P,
    ) -> Map<String, Value>
    where
        P: JsonCodec<P>,
    {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "category", &category);
        JsonCodecHelper::encode_string_field(&mut obj, "id", &id);
        obj.insert("param".to_string(), Value::String(param.encode_string()));

        obj
    }
}

impl JsonCodec<Self> for RouterWSEventEmitParam {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "category", &self.category);
        JsonCodecHelper::encode_string_field(&mut obj, "id", &self.id);
        JsonCodecHelper::encode_string_field(&mut obj, "param", &self.param);

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            category: JsonCodecHelper::decode_string_field(req_obj, "category")?,
            id: JsonCodecHelper::decode_string_field(req_obj, "id")?,
            param: JsonCodecHelper::decode_string_field(req_obj, "param")?,
        })
    }
}

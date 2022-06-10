use super::super::http::*;
use crate::router_handler::*;
use cyfs_base::*;

use serde_json::{Map, Value};


#[derive(Debug)]
pub struct RouterWSAddHandlerParam {
    pub chain: RouterHandlerChain,
    pub category: RouterHandlerCategory,
    pub id: String,
    pub dec_id: Option<ObjectId>,

    pub param: RouterAddHandlerParam,
}

pub struct RouterWSRemoveHandlerParam {
    pub chain: RouterHandlerChain,
    pub category: RouterHandlerCategory,

    pub id: String,
    pub dec_id: Option<ObjectId>,
}

impl JsonCodec<RouterWSAddHandlerParam> for RouterWSAddHandlerParam {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "chain", &self.chain);
        JsonCodecHelper::encode_string_field(&mut obj, "category", &self.category);
        JsonCodecHelper::encode_string_field(&mut obj, "id", &self.id);
        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_id", self.dec_id.as_ref());
        JsonCodecHelper::encode_field(&mut obj, "param", &self.param);

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            chain: JsonCodecHelper::decode_string_field(req_obj, "chain")?,
            category: JsonCodecHelper::decode_string_field(req_obj, "category")?,
            id: JsonCodecHelper::decode_string_field(req_obj, "id")?,
            dec_id: JsonCodecHelper::decode_option_string_field(req_obj, "dec_id")?,
            param: JsonCodecHelper::decode_field(req_obj, "param")?,
        })
    }
}

impl JsonCodec<RouterWSRemoveHandlerParam> for RouterWSRemoveHandlerParam {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "chain", &self.chain);
        JsonCodecHelper::encode_string_field(&mut obj, "category", &self.category);
        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_id", self.dec_id.as_ref());
        JsonCodecHelper::encode_string_field(&mut obj, "id", &self.id);

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            chain: JsonCodecHelper::decode_string_field(req_obj, "chain")?,
            category: JsonCodecHelper::decode_string_field(req_obj, "category")?,
            id: JsonCodecHelper::decode_string_field(req_obj, "id")?,
            dec_id: JsonCodecHelper::decode_option_string_field(req_obj, "dec_id")?,
        })
    }
}

#[derive(Debug)]
pub struct RouterWSHandlerResponse {
    pub err: u32,
    pub msg: Option<String>,
}

impl JsonCodec<Self> for RouterWSHandlerResponse {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert("err".to_owned(), Value::String(self.err.to_string()));
        if self.msg.is_some() {
            obj.insert(
                "msg".to_owned(),
                Value::String(self.msg.as_ref().unwrap().clone()),
            );
        }

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        let mut err: Option<u32> = None;
        let mut msg: Option<String> = None;

        for (k, v) in obj {
            match k.as_str() {
                "err" => {
                    let v = v.as_str().unwrap_or("").parse::<u32>().map_err(|e| {
                        error!("parse err field error: {} {:?}", e, obj);

                        BuckyError::from(BuckyErrorCode::InvalidFormat)
                    })?;

                    err = Some(v);
                }

                "msg" => {
                    let v = v.as_str().unwrap_or("");
                    msg = Some(v.to_owned());
                }

                u @ _ => {
                    error!("unknown handler register response field: {}", u);
                }
            }
        }

        if err.is_none() {
            error!("err field missing! {:?}", obj);
            return Err(BuckyError::from(BuckyErrorCode::InvalidFormat));
        }

        Ok(Self {
            err: err.unwrap(),
            msg,
        })
    }
}


pub struct RouterWSHandlerEventParam {
    pub chain: RouterHandlerChain,
    pub category: RouterHandlerCategory,

    pub id: String,

    pub param: String,
}

// pub type RouterWSHandlerEventResponse = RouterHandlerAnyResponse;

impl RouterWSHandlerEventParam {
    pub fn encode_json_impl<P>(
        chain: &RouterHandlerChain,
        category: &RouterHandlerCategory,
        id: &str,
        param: &P,
    ) -> Map<String, Value>
    where
        P: JsonCodec<P>,
    {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "chain", &chain);
        JsonCodecHelper::encode_string_field(&mut obj, "category", &category);
        JsonCodecHelper::encode_string_field(&mut obj, "id", &id);
        obj.insert("param".to_string(), Value::String(param.encode_string()));

        obj
    }
}

impl JsonCodec<RouterWSHandlerEventParam> for RouterWSHandlerEventParam {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_string_field(&mut obj, "chain", &self.chain);
        JsonCodecHelper::encode_string_field(&mut obj, "category", &self.category);
        JsonCodecHelper::encode_string_field(&mut obj, "id", &self.id);
        JsonCodecHelper::encode_string_field(&mut obj, "param", &self.param);

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            chain: JsonCodecHelper::decode_string_field(req_obj, "chain")?,
            category: JsonCodecHelper::decode_string_field(req_obj, "category")?,
            id: JsonCodecHelper::decode_string_field(req_obj, "id")?,
            param: JsonCodecHelper::decode_string_field(req_obj, "param")?,
        })
    }
}

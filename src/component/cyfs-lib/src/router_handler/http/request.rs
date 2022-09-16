use cyfs_base::*;
use super::super::*;

use serde_json::{Map, Value};

#[derive(Debug)]
pub struct RouterAddHandlerParam {
    pub filter: String,
    pub req_path: Option<String>,
    pub index: i32,

    pub default_action: RouterHandlerAction,

    pub routine: Option<String>,
}

pub struct RouterRemoveHandlerParam {
    pub id: String,
}

impl JsonCodec<RouterAddHandlerParam> for RouterAddHandlerParam {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert(
            "filter".to_string(),
            Value::String(self.filter.clone()),
        );

        JsonCodecHelper::encode_option_string_field(&mut obj, "req_path", self.req_path.as_ref());

        obj.insert(
            "index".to_string(),
            Value::String(self.index.to_string()),
        );
        obj.insert(
            "default_action".to_string(),
            Value::Object(self.default_action.encode_json()),
        );

        if self.routine.is_some() {
            obj.insert(
                "routine".to_string(),
                Value::String(self.routine.as_ref().unwrap().clone()),
            );
        }

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        let mut filter: Option<String> = None;
        let mut req_path: Option<String> = None;
        let mut default_action: Option<RouterHandlerAction> = None;
        let mut routine: Option<String> = None;
        let mut index: Option<i32> = None;

        for (k, v) in req_obj {
            match k.as_str() {
                "filter" => {
                    filter = Some(JsonCodecHelper::decode_from_string(&v)?);
                }
                "req_path" => {
                    req_path = Some(JsonCodecHelper::decode_from_string(&v)?);
                }

                "index" => {
                    index = Some(JsonCodecHelper::decode_to_int(v)?);
                }
                "default_action" => {
                    /*
                    支持两种模式
                    default_action: 'xxxx',
                    或者
                    default_action: {
                        "action": "xxxx",
                    }
                    */
                    if v.is_string() {
                        default_action = Some(JsonCodecHelper::decode_from_string(v)?);
                    } else if v.is_object() {
                        default_action = Some(RouterHandlerAction::decode_json(v.as_object().unwrap())?);
                    } else {
                        let msg = format!("invalid default_action field format: {:?}", v);
                        warn!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                    }
                }

                "routine" => {
                    if !v.is_string() {
                        let msg = format!("invalid routine field: {:?}", v);
                        warn!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                    }

                    routine = Some(v.as_str().unwrap().to_owned());
                }

                u @ _ => {
                    warn!("unknown router handler field: {}", u);
                }
            }
        }

        if filter.is_none() || index.is_none() || default_action.is_none() {
            let msg = format!("router handler request field missing: filter/default_action");
            warn!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let req = Self {
            filter: filter.unwrap(),
            req_path,
            index: index.unwrap(),
            default_action: default_action.unwrap(),
            routine,
        };

        Ok(req)
    }
}

impl JsonCodec<RouterRemoveHandlerParam> for RouterRemoveHandlerParam {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert("id".to_string(), Value::String(self.id.clone()));

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        let mut id: Option<String> = None;

        for (k, v) in req_obj {
            match k.as_str() {
                "id" => {
                    if !v.is_string() {
                        let msg = format!("invalid handler id field: {:?}", v);
                        warn!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                    }

                    id = Some(v.as_str().unwrap().to_owned())
                }

                u @ _ => {
                    warn!("unknown router handler field: {}", u);
                }
            }
        }

        if id.is_none() {
            let msg = format!("routine handler request missing: id");
            warn!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let req = Self { id: id.unwrap() };

        Ok(req)
    }
}

/*
#[derive(Debug)]
pub struct RouterHandlerResponse {
    pub err: u32,
    pub msg: Option<String>,
}

impl JsonCodec<RouterHandlerResponse> for RouterHandlerResponse {
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
*/
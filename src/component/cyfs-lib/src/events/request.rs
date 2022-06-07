use crate::*;
use cyfs_base::*;

use serde_json::{Map, Value};
use std::fmt;

pub struct RouterEventRequest<REQ>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
{
    pub request: REQ,
}

impl<REQ> fmt::Display for RouterEventRequest<REQ>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "request: {}", self.request)?;

        Ok(())
    }
}

impl<REQ> RouterEventCategoryInfo for RouterEventRequest<REQ>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display + RouterEventCategoryInfo,
{
    fn category() -> RouterEventCategory {
        extract_router_event_category::<REQ>()
    }
}

pub struct RouterEventResponse<RESP>
where
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    pub handled: bool,
    pub call_next: bool,
    pub response: Option<BuckyResult<RESP>>,
}

impl<RESP> fmt::Display for RouterEventResponse<RESP>
where
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "handled: {}", self.handled)?;
        write!(f, "call_next: {}", self.call_next)?;

        if let Some(resp) = &self.response {
            match resp {
                Ok(v) => write!(f, "response: {}", v)?,
                Err(e) => write!(f, "response error: {}", e)?,
            }
        }

        Ok(())
    }
}

impl<REQ> JsonCodec<RouterEventRequest<REQ>> for RouterEventRequest<REQ>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
{
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert("request".to_string(), self.request.encode_value());

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            request: JsonCodecHelper::decode_field(req_obj, "request")?,
        })
    }
}

impl<RESP> JsonCodec<RouterEventResponse<RESP>> for RouterEventResponse<RESP>
where
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert("handled".to_string(), Value::Bool(self.handled));
        obj.insert("call_next".to_string(), Value::Bool(self.call_next));

        if let Some(resp) = &self.response {
            obj.insert("response".to_string(), resp.encode_value());
        }

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            handled: JsonCodecHelper::decode_bool_field(req_obj, "handled")?,
            call_next: JsonCodecHelper::decode_bool_field(req_obj, "call_next")?,
            response: JsonCodecHelper::decode_option_field(req_obj, "response")?,
        })
    }
}

pub struct RouterEventResponseHelper;

impl RouterEventResponseHelper {
    pub fn encode_default() -> String {
        RouterEventResponse::<TestEventRequest> {
            handled: false,
            call_next: true,
            response: None,
        }
        .encode_string()
    }
}

// test event
crate::declare_event_empty_param!(TestEventRequest, TestEvent);
crate::declare_event_empty_param!(TestEventResponse, TestEvent);

// request
pub type RouterEventTestEventRequest = RouterEventRequest<TestEventRequest>;

// response
pub type RouterEventTestEventResult = RouterEventResponse<TestEventResponse>;

use super::action::*;
use crate::*;
use cyfs_base::*;

use serde_json::{Map, Value};
use std::fmt;

pub struct RouterHandlerRequest<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    pub request: REQ,
    pub response: Option<BuckyResult<RESP>>,
}

impl<REQ, RESP> fmt::Display for RouterHandlerRequest<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "request: {}", self.request)?;

        if let Some(resp) = &self.response {
            match resp {
                Ok(v) => write!(f, ", response: {}", v)?,
                Err(e) => write!(f, ", response error: {}", e)?,
            }
        }

        Ok(())
    }
}

pub struct RouterHandlerResponse<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    pub action: RouterHandlerAction,
    pub request: Option<REQ>,
    pub response: Option<BuckyResult<RESP>>,
}

impl<REQ, RESP> fmt::Display for RouterHandlerResponse<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "action: {}", self.action)?;

        if let Some(req) = &self.request {
            write!(f, ", request: {}", req)?;
        }

        if let Some(resp) = &self.response {
            match resp {
                Ok(v) => write!(f, "response: {}", v)?,
                Err(e) => write!(f, "response error: {}", e)?,
            }
        }

        Ok(())
    }
}

// request
pub type RouterHandlerPutObjectRequest =
    RouterHandlerRequest<NONPutObjectInputRequest, NONPutObjectInputResponse>;
pub type RouterHandlerGetObjectRequest =
    RouterHandlerRequest<NONGetObjectInputRequest, NONGetObjectInputResponse>;
pub type RouterHandlerPostObjectRequest =
    RouterHandlerRequest<NONPostObjectInputRequest, NONPostObjectInputResponse>;
pub type RouterHandlerSelectObjectRequest =
    RouterHandlerRequest<NONSelectObjectInputRequest, NONSelectObjectInputResponse>;
pub type RouterHandlerDeleteObjectRequest =
    RouterHandlerRequest<NONDeleteObjectInputRequest, NONDeleteObjectInputResponse>;

pub type RouterHandlerPutDataRequest =
    RouterHandlerRequest<NDNPutDataInputRequest, NDNPutDataInputResponse>;
pub type RouterHandlerGetDataRequest =
    RouterHandlerRequest<NDNGetDataInputRequest, NDNGetDataInputResponse>;
pub type RouterHandlerDeleteDataRequest =
    RouterHandlerRequest<NDNDeleteDataInputRequest, NDNDeleteDataInputResponse>;

pub type RouterHandlerSignObjectRequest =
    RouterHandlerRequest<CryptoSignObjectInputRequest, CryptoSignObjectInputResponse>;
pub type RouterHandlerVerifyObjectRequest =
    RouterHandlerRequest<CryptoVerifyObjectInputRequest, CryptoVerifyObjectInputResponse>;
pub type RouterHandlerEncryptDataRequest =
    RouterHandlerRequest<CryptoEncryptDataInputRequest, CryptoEncryptDataInputResponse>;
pub type RouterHandlerDecryptDataRequest =
    RouterHandlerRequest<CryptoDecryptDataInputRequest, CryptoDecryptDataInputResponse>;

pub type RouterHandlerAclRequest =
    RouterHandlerRequest<AclHandlerRequest, AclHandlerResponse>;

pub type RouterHandlerInterestRequest = 
    RouterHandlerRequest<InterestHandlerRequest, InterestHandlerResponse>;

// response
pub type RouterHandlerPutObjectResult =
    RouterHandlerResponse<NONPutObjectInputRequest, NONPutObjectInputResponse>;
pub type RouterHandlerGetObjectResult =
    RouterHandlerResponse<NONGetObjectInputRequest, NONGetObjectInputResponse>;
pub type RouterHandlerPostObjectResult =
    RouterHandlerResponse<NONPostObjectInputRequest, NONPostObjectInputResponse>;
pub type RouterHandlerSelectObjectResult =
    RouterHandlerResponse<NONSelectObjectInputRequest, NONSelectObjectInputResponse>;
pub type RouterHandlerDeleteObjectResult =
    RouterHandlerResponse<NONDeleteObjectInputRequest, NONDeleteObjectInputResponse>;

pub type RouterHandlerPutDataResult =
    RouterHandlerResponse<NDNPutDataInputRequest, NDNPutDataInputResponse>;
pub type RouterHandlerGetDataResult =
    RouterHandlerResponse<NDNGetDataInputRequest, NDNGetDataInputResponse>;
pub type RouterHandlerDeleteDataResult =
    RouterHandlerResponse<NDNDeleteDataInputRequest, NDNDeleteDataInputResponse>;

pub type RouterHandlerSignObjectResult =
    RouterHandlerResponse<CryptoSignObjectInputRequest, CryptoSignObjectInputResponse>;
pub type RouterHandlerVerifyObjectResult =
    RouterHandlerResponse<CryptoVerifyObjectInputRequest, CryptoVerifyObjectInputResponse>;
pub type RouterHandlerEncryptDataResult =
    RouterHandlerResponse<CryptoEncryptDataInputRequest, CryptoEncryptDataInputResponse>;
pub type RouterHandlerDecryptDataResult =
    RouterHandlerResponse<CryptoDecryptDataInputRequest, CryptoDecryptDataInputResponse>;

pub type RouterHandlerAclResult =
    RouterHandlerResponse<AclHandlerRequest, AclHandlerResponse>;

pub type RouterHandlerInterestResult = 
    RouterHandlerResponse<InterestHandlerRequest, InterestHandlerResponse>;

pub struct RouterHandlerResponseHelper;

impl RouterHandlerResponseHelper {
    pub fn encode_with_action(action: RouterHandlerAction) -> String {
        RouterHandlerResponse::<NONPutObjectInputRequest, NONPutObjectInputResponse> {
            action,
            request: None,
            response: None,
        }
        .encode_string()
    }
}

impl<REQ, RESP> JsonCodec<RouterHandlerRequest<REQ, RESP>> for RouterHandlerRequest<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert("request".to_string(), self.request.encode_value());
        if let Some(resp) = &self.response {
            obj.insert("response".to_string(), resp.encode_value());
        }

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            request: JsonCodecHelper::decode_field(req_obj, "request")?,
            response: JsonCodecHelper::decode_option_field(req_obj, "response")?,
        })
    }
}

impl<REQ, RESP> JsonCodec<RouterHandlerResponse<REQ, RESP>> for RouterHandlerResponse<REQ, RESP>
where
    REQ: Send + Sync + 'static + JsonCodec<REQ> + fmt::Display,
    RESP: Send + Sync + 'static + JsonCodec<RESP> + fmt::Display,
{
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert("action".to_string(), Value::String(self.action.to_string()));
        if let Some(req) = &self.request {
            obj.insert("request".to_string(), req.encode_value());
        }
        if let Some(resp) = &self.response {
            obj.insert("response".to_string(), resp.encode_value());
        }

        obj
    }

    fn decode_json(req_obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            action: JsonCodecHelper::decode_string_field(req_obj, "action")?,
            request: JsonCodecHelper::decode_option_field(req_obj, "request")?,
            response: JsonCodecHelper::decode_option_field(req_obj, "response")?,
        })
    }
}

use cyfs_base::*;

use async_std::net::{SocketAddr, TcpStream};
use http_types::{
    headers::{HeaderName, HeaderValue, HeaderValues, ToHeaderValues},
    Body, Mime, Request, Response, StatusCode,
};
use std::borrow::Cow;
use std::str::FromStr;

#[async_trait::async_trait]
pub trait BodyOp {
    async fn body_bytes(&mut self) -> http_types::Result<Vec<u8>>;
    async fn body_string(&mut self) -> http_types::Result<String>;
    fn set_body(&mut self, body: impl Into<Body>);
    fn set_content_type(&mut self, mime: Mime) -> Option<HeaderValues>;
}

#[async_trait::async_trait]
impl BodyOp for Request {
    async fn body_bytes(&mut self) -> http_types::Result<Vec<u8>> {
        self.body_bytes().await
    }

    async fn body_string(&mut self) -> http_types::Result<String> {
        self.body_string().await
    }

    fn set_body(&mut self, body: impl Into<Body>) {
        self.set_body(body);
    }

    fn set_content_type(&mut self, mime: Mime) -> Option<HeaderValues> {
        self.set_content_type(mime)
    }
}

#[async_trait::async_trait]
impl<State: Send> BodyOp for ::tide::Request<State> {
    async fn body_bytes(&mut self) -> http_types::Result<Vec<u8>> {
        self.body_bytes().await
    }

    async fn body_string(&mut self) -> http_types::Result<String> {
        self.body_string().await
    }

    fn set_body(&mut self, body: impl Into<Body>) {
        self.set_body(body);
    }

    fn set_content_type(&mut self, mime: Mime) -> Option<HeaderValues> {
        AsMut::<Request>::as_mut(self).set_content_type(mime)
    }
}

#[async_trait::async_trait]
impl BodyOp for Response {
    async fn body_bytes(&mut self) -> http_types::Result<Vec<u8>> {
        self.body_bytes().await
    }
    async fn body_string(&mut self) -> http_types::Result<String> {
        self.body_string().await
    }

    fn set_body(&mut self, body: impl Into<Body>) {
        self.set_body(body);
    }
    fn set_content_type(&mut self, mime: Mime) -> Option<HeaderValues> {
        self.set_content_type(mime)
    }
}

#[async_trait::async_trait]
impl BodyOp for ::tide::Response {
    async fn body_bytes(&mut self) -> http_types::Result<Vec<u8>> {
        self.body_bytes().await
    }
    async fn body_string(&mut self) -> http_types::Result<String> {
        self.body_string().await
    }

    fn set_body(&mut self, body: impl Into<Body>) {
        self.set_body(body);
    }
    fn set_content_type(&mut self, mime: Mime) -> Option<HeaderValues> {
        AsMut::<Response>::as_mut(self).set_content_type(mime)
    }
}

pub trait HeaderOp {
    fn header(&self, name: impl Into<HeaderName>) -> Option<&HeaderValues>;
    fn insert_header(
        &mut self,
        name: impl Into<HeaderName>,
        values: impl ToHeaderValues,
    ) -> Option<HeaderValues>;
}

impl HeaderOp for Request {
    fn header(&self, name: impl Into<HeaderName>) -> Option<&HeaderValues> {
        Request::header(&self, name)
    }

    fn insert_header(
        &mut self,
        name: impl Into<HeaderName>,
        values: impl ToHeaderValues,
    ) -> Option<HeaderValues> {
        Request::insert_header(self, name, values)
    }
}

impl HeaderOp for Response {
    fn header(&self, name: impl Into<HeaderName>) -> Option<&HeaderValues> {
        Response::header(&self, name)
    }

    fn insert_header(
        &mut self,
        name: impl Into<HeaderName>,
        values: impl ToHeaderValues,
    ) -> Option<HeaderValues> {
        Response::insert_header(self, name, values)
    }
}

impl<State> HeaderOp for ::tide::Request<State> {
    fn header(&self, name: impl Into<HeaderName>) -> Option<&HeaderValues> {
        ::tide::Request::header(&self, name)
    }

    fn insert_header(
        &mut self,
        name: impl Into<HeaderName>,
        values: impl ToHeaderValues,
    ) -> Option<HeaderValues> {
        ::tide::Request::insert_header(self, name, values)
    }
}

impl HeaderOp for ::tide::Response {
    fn header(&self, name: impl Into<HeaderName>) -> Option<&HeaderValues> {
        ::tide::Response::header(&self, name)
    }

    fn insert_header(
        &mut self,
        name: impl Into<HeaderName>,
        values: impl ToHeaderValues,
    ) -> Option<HeaderValues> {
        let resp: &mut Response = self.as_mut();
        resp.insert_header(name, values)
    }
}

struct BuckyErrorStatusCodeTrans {}

impl BuckyErrorStatusCodeTrans {
    pub fn bucky_error_to_status_code(e: BuckyErrorCode) -> StatusCode {
        match e {
            BuckyErrorCode::Ok => StatusCode::NoContent,
            BuckyErrorCode::InvalidFormat
            | BuckyErrorCode::InvalidInput
            | BuckyErrorCode::InvalidParam
            | BuckyErrorCode::InvalidData => StatusCode::BadRequest,

            BuckyErrorCode::NotFound | BuckyErrorCode::InnerPathNotFound => StatusCode::NotFound,
            BuckyErrorCode::PermissionDenied => StatusCode::Forbidden,
            BuckyErrorCode::Unknown => StatusCode::InternalServerError,
            BuckyErrorCode::ErrorState | BuckyErrorCode::Timeout => StatusCode::GatewayTimeout,
            BuckyErrorCode::ConnectFailed | BuckyErrorCode::ConnectInterZoneFailed => {
                StatusCode::GatewayTimeout
            }
            BuckyErrorCode::Reject => StatusCode::Forbidden,
            BuckyErrorCode::Ignored => StatusCode::NotAcceptable,
            BuckyErrorCode::NotHandled => StatusCode::NotImplemented,
            BuckyErrorCode::RangeNotSatisfiable => StatusCode::RequestedRangeNotSatisfiable,
            _ => {
                warn!("unknown error code: {}", e);
                StatusCode::InternalServerError
            }
        }
    }

    pub fn status_code_to_bucky_error(code: StatusCode) -> BuckyErrorCode {
        if code.is_success() {
            return BuckyErrorCode::Ok;
        }

        match code {
            StatusCode::BadRequest => BuckyErrorCode::InvalidData,
            StatusCode::NotFound => BuckyErrorCode::NotFound,
            StatusCode::Forbidden => BuckyErrorCode::Reject,
            StatusCode::GatewayTimeout => BuckyErrorCode::Timeout,
            StatusCode::NotAcceptable => BuckyErrorCode::Ignored,
            StatusCode::InternalServerError => BuckyErrorCode::Unknown,
            StatusCode::NotImplemented => BuckyErrorCode::NotHandled,
            StatusCode::RequestedRangeNotSatisfiable => BuckyErrorCode::RangeNotSatisfiable,
            _ => BuckyErrorCode::Unknown,
        }
    }
}

pub struct RequestorHelper;

impl RequestorHelper {
    pub fn new_response(code: StatusCode) -> Response {
        let resp = Response::new(code);
        //resp.insert_header("Access-Control-Allow-Origin", "*");

        resp
    }

    pub fn new_ok_response<R>() -> R
    where
        R: From<Response>,
    {
        let resp = Response::new(StatusCode::Ok);

        resp.into()
    }

    pub fn trans_error<R>(e: BuckyError) -> R
    where
        R: From<Response>,
    {
        let code = BuckyErrorStatusCodeTrans::bucky_error_to_status_code(e.code());

        let mut resp = Self::new_response(code);

        // always encode error content to body
        let body = e.encode_string();
        resp.set_content_type(tide::http::mime::JSON);
        resp.set_body(body);

        resp.into()
    }

    // 从body里面提取buckyerror
    // 对于status是success情况下，一律解析为BuckyErrorCode
    pub async fn error_from_resp(resp: &mut Response) -> BuckyError {
        // assert!(!resp.status().is_success());

        let err_code = BuckyErrorStatusCodeTrans::status_code_to_bucky_error(resp.status());

        // 尝试从body里面读取编码后的BuckyError
        let body = resp.body_string().await.map_err(|e| {
            let msg = format!("read error string from response error: {}", e);
            error!("{}", msg);

            BuckyError::from(err_code)
        });
        if let Err(e) = body {
            return e;
        }

        let body = body.unwrap();
        if body.is_empty() {
            BuckyError::from(err_code)
        } else {
            match BuckyError::decode_string(&body) {
                Ok(e) => e,
                Err(e) => {
                    error!("invalid error string from response: {}, {}", body, e);
                    BuckyError::new(err_code, body)
                }
            }
        }
    }

    pub fn insert_device_list_header(http_req: &mut Request, device_list: &Vec<DeviceId>) {
        let device_list: Vec<HeaderValue> = device_list
            .iter()
            .map(|device_id| HeaderValue::from_str(&device_id.to_string()).unwrap())
            .collect();

        http_req.insert_header(::cyfs_base::CYFS_DEVICE_ID, &device_list[..]);
    }

    pub fn insert_headers<T>(http_req: &mut Request, name: &str, list: &Vec<T>)
    where
        T: ToString,
    {
        let header_list: Vec<HeaderValue> = list
            .iter()
            .map(|item| HeaderValue::from_str(&item.to_string()).unwrap())
            .collect();

        http_req.insert_header(name, &header_list[..]);
    }

    pub fn trans_status_code(code: StatusCode) -> BuckyErrorCode {
        if code.is_success() {
            return BuckyErrorCode::Ok;
        }

        match code {
            StatusCode::NotFound => BuckyErrorCode::NotFound,
            StatusCode::BadRequest => BuckyErrorCode::InvalidParam,
            StatusCode::Forbidden | StatusCode::Unauthorized => BuckyErrorCode::PermissionDenied,
            _ => BuckyErrorCode::Failed,
        }
    }

    pub fn decode_optional_hex_header<R>(req: &R, name: &str) -> BuckyResult<Option<Vec<u8>>>
    where
        R: HeaderOp,
    {
        let mut ret: Option<Vec<u8>> = None;
        if let Some(header) = req.header(name) {
            let value = header.last().as_str();
            let value = hex::decode(value).map_err(|e| {
                let msg = format!("invalid header hex format: {} {}", value, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

            ret = Some(value);
        }

        Ok(ret)
    }

    pub fn decode_optional_header<T, R>(
        req: &R,
        name: impl Into<HeaderName>,
    ) -> BuckyResult<Option<T>>
    where
        R: HeaderOp,
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let mut ret: Option<T> = None;
        let name: HeaderName = name.into();
        if let Some(header) = req.header(&name) {
            let value = header.last().as_str();
            let value = T::from_str(value).map_err(|e| {
                let msg = format!("invalid header format: {} = {} {}", name, value, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

            ret = Some(value);
        }

        Ok(ret)
    }

    pub fn decode_utf8(name: &str, value: &str) -> BuckyResult<String> {
        let decoded_value = percent_encoding::percent_decode_str(&value);
        let value = decoded_value.decode_utf8().map_err(|e| {
            let msg = format!("invalid header format: {} = {} {}", name, value, e);
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(value.to_string())
    }

    pub fn decode_optional_header_with_utf8_decoding<R>(
        req: &R,
        name: impl Into<HeaderName>,
    ) -> BuckyResult<Option<String>>
    where
        R: HeaderOp,
    {
        let mut ret = None;
        let name: HeaderName = name.into();
        if let Some(header) = req.header(&name) {
            let value = header.last().as_str();
            ret = Some(Self::decode_utf8(name.as_str(), value)?);
        }

        Ok(ret)
    }

    pub fn decode_header<T, R>(req: &R, name: impl Into<HeaderName>) -> BuckyResult<T>
    where
        R: HeaderOp,
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let name: HeaderName = name.into();
        match Self::decode_optional_header(req, &name)? {
            Some(v) => Ok(v),
            None => {
                let msg = format!("header not found: {}", name);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }

    pub fn decode_header_with_utf8_decoding<R>(
        req: &R,
        name: impl Into<HeaderName>,
    ) -> BuckyResult<String>
    where
        R: HeaderOp,
    {
        let name: HeaderName = name.into();
        match Self::decode_optional_header_with_utf8_decoding(req, &name)? {
            Some(v) => Ok(v),
            None => {
                let msg = format!("header not found: {}", name);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }

    pub fn decode_optional_headers<T, R>(
        req: &R,
        name: impl Into<HeaderName>,
    ) -> BuckyResult<Option<Vec<T>>>
    where
        R: HeaderOp,
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let name: HeaderName = name.into();
        if let Some(headers) = req.header(&name) {
            let mut rets = Vec::new();
            for item in headers {
                let value = T::from_str(item.as_str()).map_err(|e| {
                    let msg = format!("invalid header format: {} = {}, {}", name, item, e);
                    error!("{}", msg);

                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

                rets.push(value);
            }

            Ok(Some(rets))
        } else {
            Ok(None)
        }
    }

    pub fn decode_headers<T, R>(req: &R, name: impl Into<HeaderName>) -> BuckyResult<Vec<T>>
    where
        R: HeaderOp,
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let name: HeaderName = name.into();
        match Self::decode_optional_headers(req, &name)? {
            Some(v) => Ok(v),
            None => {
                let msg = format!("header not found: {}", name);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }

    pub fn decode_json_header<T, R>(req: &R, name: impl Into<HeaderName>) -> BuckyResult<T>
    where
        R: HeaderOp,
        T: JsonCodec<T>,
    {
        let name: HeaderName = name.into();
        match Self::decode_optional_json_header(req, &name)? {
            Some(v) => Ok(v),
            None => {
                let msg = format!("header not found: {}", name);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
            }
        }
    }

    pub fn decode_optional_json_header<T, R>(
        req: &R,
        name: impl Into<HeaderName>,
    ) -> BuckyResult<Option<T>>
    where
        R: HeaderOp,
        T: JsonCodec<T>,
    {
        let mut ret: Option<T> = None;
        let name: HeaderName = name.into();
        if let Some(header) = req.header(&name) {
            let value = header.last().as_str();
            let value = T::decode_string(value).map_err(|e| {
                let msg = format!("invalid header json format: {} {}", value, e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

            ret = Some(value);
        }

        Ok(ret)
    }

    pub fn decode_optional_json_headers<T, R>(
        req: &R,
        name: impl Into<HeaderName>,
    ) -> BuckyResult<Option<Vec<T>>>
    where
        R: HeaderOp,
        T: JsonCodec<T>,
    {
        let name: HeaderName = name.into();
        if let Some(headers) = req.header(&name) {
            let mut rets = Vec::new();
            for item in headers {
                let value = T::decode_string(&item.as_str()).map_err(|e| {
                    let msg = format!("invalid header json format: {} {}", item.as_str(), e);
                    error!("{}", msg);

                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

                rets.push(value);
            }

            Ok(Some(rets))
        } else {
            Ok(None)
        }
    }

    pub async fn decode_raw_object_body<R, T>(body: &mut R) -> BuckyResult<T>
    where
        R: BodyOp,
        T: for<'d> RawDecode<'d>,
    {
        let buf = body.body_bytes().await.map_err(|e| {
            let msg = format!("read object raw body bytes error! {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let (object, _) = T::raw_decode(&buf).map_err(|e| {
            error!("decode object from raw bytes error: {}", e,);
            e
        })?;

        Ok(object)
    }

    pub async fn decode_str_body<R, T>(body: &mut R) -> BuckyResult<T>
    where
        R: BodyOp,
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let body = body.body_string().await.map_err(|e| {
            let msg = format!("read body string error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let value = T::from_str(&body).map_err(|e| {
            let msg = format!("invalid body format: {} {}", body, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(value)
    }

    pub async fn decode_json_body<R, T>(resp: &mut R) -> BuckyResult<T>
    where
        R: BodyOp,
        T: JsonCodec<T>,
    {
        let body = resp.body_string().await.map_err(|e| {
            let msg = format!("read body string error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let value = T::decode_string(&body).map_err(|e| {
            let msg = format!("invalid json body format: {} {}", body, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(value)
    }

    pub fn decode_url_param<T>(k: Cow<str>, v: Cow<str>) -> BuckyResult<T>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let value = T::from_str(v.as_ref()).map_err(|e| {
            let msg = format!("invalid url param: {} = {}, {}", k, v, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(value)
    }

    // 参数使用,分隔
    pub fn decode_url_param_list<T>(k: Cow<str>, v: Cow<str>) -> BuckyResult<Vec<T>>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        let mut list = vec![];
        for param in v.split(",") {
            let ret = Self::decode_url_param(k.clone(), std::borrow::Cow::Borrowed(param))?;
            list.push(ret);
        }

        Ok(list)
    }

    pub fn encode_opt_header<T, R>(req: &mut R, name: &str, value: &Option<T>)
    where
        R: HeaderOp,
        T: ToString,
    {
        if let Some(value) = value {
            req.insert_header(name, value.to_string());
        }
    }

    pub fn encode_opt_header_with_encoding<R>(req: &mut R, name: &str, value: Option<&str>)
    where
        R: HeaderOp,
    {
        if let Some(value) = value {
            let v: String =
                percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC)
                    .collect();
            req.insert_header(name, v);
        }
    }

    pub fn encode_header<T, R>(req: &mut R, name: &str, value: &T)
    where
        R: HeaderOp,
        T: ToString,
    {
        req.insert_header(name, value.to_string());
    }

    pub fn encode_header_with_encoding<R>(req: &mut R, name: &str, value: &str)
    where
        R: HeaderOp,
    {
        let v: String =
            percent_encoding::utf8_percent_encode(value, percent_encoding::NON_ALPHANUMERIC)
                .collect();
        req.insert_header(name, v);
    }

    pub fn encode_time_header<R>(req: &mut R, name: impl Into<HeaderName>, bucky_time: u64)
    where
        R: HeaderOp,
    {
        use chrono::{DateTime, NaiveDateTime, Utc};

        let unix_time = cyfs_base::bucky_time_to_unix_time(bucky_time);
        let secs = unix_time / (1000 * 1000);
        let nsecs = if secs > 0 {
            (unix_time % secs) * 1000 
        } else {
            0
        };

        let time = NaiveDateTime::from_timestamp(secs as i64, nsecs as u32);

        let dt = DateTime::<Utc>::from_utc(time, Utc);
        let s = dt.to_rfc2822();

        req.insert_header(name, s);
    }

    pub async fn request_to_service(
        service_addr: &SocketAddr,
        req: Request,
    ) -> BuckyResult<Response> {
        debug!(
            "will request to non service: {} {}",
            req.method(),
            req.url()
        );

        let tcp_stream = TcpStream::connect(service_addr).await.map_err(|e| {
            let msg = format!("connect to non service error: {} {}", service_addr, e);
            error!("{}", msg);

            BuckyError::from(e)
        })?;

        match async_h1::connect(tcp_stream, req).await {
            Ok(resp) => {
                info!("request to non service success! {}", service_addr);
                Ok(resp)
            }
            Err(e) => {
                let msg = format!("request to non service failed! {} {}", service_addr, e);
                error!("{}", msg);

                Err(BuckyError::from(msg))
            }
        }
    }

    pub fn value_from_querys<T>(name: &str, url: &http_types::Url) -> BuckyResult<Option<T>>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        match url.query_pairs().find(|(x, _)| x == name) {
            Some((_, v)) => {
                let v = T::from_str(v.as_ref()).map_err(|e| {
                    let msg = format!("invalid query in url: {}={}, {}", name, v, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidParam, msg)
                })?;

                Ok(Some(v))
            }
            _ => Ok(None),
        }
    }
}

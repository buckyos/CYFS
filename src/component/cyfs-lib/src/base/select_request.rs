use crate::*;
use cyfs_base::*;

use http_types::{Method, Request, Response, StatusCode, Url};
use serde_json::{Map, Value};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone)]
pub struct SelectTimeRange {
    // [begin, end)
    pub begin: Option<u64>,
    pub end: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SelectFilter {
    pub obj_type: Option<u16>,
    pub obj_type_code: Option<ObjectTypeCode>,

    pub dec_id: Option<ObjectId>,
    pub owner_id: Option<ObjectId>,
    pub author_id: Option<ObjectId>,

    pub create_time: Option<SelectTimeRange>,
    pub update_time: Option<SelectTimeRange>,
    pub insert_time: Option<SelectTimeRange>,

    // TODO 目前flags只支持全匹配
    pub flags: Option<u32>,
}

impl Default for SelectFilter {
    fn default() -> Self {
        Self {
            obj_type: None,
            obj_type_code: None,

            dec_id: None,
            owner_id: None,
            author_id: None,

            create_time: None,
            update_time: None,
            insert_time: None,

            flags: None,
        }
    }
}

impl fmt::Display for SelectFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(v) = &self.obj_type {
            write!(f, "obj_type:{} ", v)?;
        }
        if let Some(v) = &self.obj_type_code {
            write!(f, "obj_type_code:{} ", v.to_u16())?;
        }

        if let Some(v) = &self.dec_id {
            write!(f, "dec_id:{} ", v.to_string())?;
        }
        if let Some(v) = &self.owner_id {
            write!(f, "owner_id:{} ", v.to_string())?;
        }
        if let Some(v) = &self.author_id {
            write!(f, "author_id:{} ", v.to_string())?;
        }

        if let Some(v) = &self.create_time {
            write!(f, "create_time:{:?} ", v)?;
        }
        if let Some(v) = &self.update_time {
            write!(f, "update_time:{:?} ", v)?;
        }
        if let Some(v) = &self.insert_time {
            write!(f, "insert_time:{:?} ", v)?;
        }

        if let Some(v) = &self.flags {
            write!(f, "flags:{} ", v)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SelectOption {
    // 每页读取的数量
    pub page_size: u16,

    // 当前读取的页码，从0开始
    pub page_index: u16,
}

impl Default for SelectOption {
    fn default() -> Self {
        Self {
            page_size: 32_u16,
            page_index: 0_u16,
        }
    }
}

impl fmt::Display for SelectOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "page_size:{} ", self.page_size)?;
        write!(f, "page_index:{}", self.page_index)
    }
}


#[derive(Debug, Clone, RawDecode, RawEncode)]
pub struct SelectResponseObjectMetaInfo {
    pub insert_time: u64,
    pub create_dec_id: Option<ObjectId>,
    pub context: Option<String>,
    pub last_access_rpath: Option<String>,
    pub access_string: Option<u32>,
}

#[derive(Debug, Clone, RawEncode, RawDecode)]
pub struct SelectResponseObjectInfo {
    pub meta: SelectResponseObjectMetaInfo,
    pub object: Option<NONObjectInfo>,
}

impl SelectResponseObjectInfo {
    fn from_meta(meta: SelectResponseObjectMetaInfo) -> Self {
        Self {
            meta,
            object: None,
        }
    }

    fn meta(&self) -> &SelectResponseObjectMetaInfo {
        &self.meta
    }
}

impl fmt::Display for SelectResponseObjectMetaInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "insert_time:{}", self.insert_time)?;

        if let Some(v) = &self.create_dec_id {
            write!(f, ", create_dec_id:{} ", v)?;
        }
        if let Some(v) = &self.context {
            write!(f, ", context:{} ", v)?;
        }
        if let Some(v) = &self.last_access_rpath {
            write!(f, ", last_access_rpath:{} ", v)?;
        }
        write!(f, ", access:{:?}", self.access_string)?;

        Ok(())
    }
}

impl fmt::Display for SelectResponseObjectInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ", self.meta)?;

        if let Some(obj) = &self.object {
            write!(f, "object:{} ", obj)?;
        }

        Ok(())
    }
}

impl JsonCodec<SelectResponseObjectMetaInfo> for SelectResponseObjectMetaInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        obj.insert(
            "insert_time".to_owned(),
            Value::String(self.insert_time.to_string()),
        );

        JsonCodecHelper::encode_option_string_field(&mut obj, "create_dec_id", self.create_dec_id.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "context", self.context.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "last_access_rpath", self.last_access_rpath.as_ref());
        JsonCodecHelper::encode_option_number_field(&mut obj, "access", self.access_string);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            insert_time: JsonCodecHelper::decode_int_field(obj, "insert_time")?,
            create_dec_id: JsonCodecHelper::decode_option_string_field(obj, "create_dec_id")?,
            context: JsonCodecHelper::decode_option_string_field(obj, "context")?,
            last_access_rpath: JsonCodecHelper::decode_option_string_field(obj, "last_access_rpath")?,
            access_string: JsonCodecHelper::decode_option_int_field(obj, "access")?,
        })
    }
}

impl JsonCodec<SelectResponseObjectInfo> for SelectResponseObjectInfo {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_field(&mut obj, "meta", &self.meta);
        let mut obj = self.meta().encode_json();
        if let Some(object) = &self.object {
            JsonCodecHelper::encode_field(&mut obj, "object", object);
        }
        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<Self> {
        Ok(Self {
            meta: JsonCodecHelper::decode_field(obj, "meta")?,
            object: JsonCodecHelper::decode_option_field(obj, "object")?,
        })
    }
}

impl SelectResponseObjectInfo {
    // 绑定对象，仅可执行一次
    pub fn bind_object(&mut self, buf: Vec<u8>) -> BuckyResult<()> {
        assert!(self.object.is_none());

        let info = NONObjectInfo::new_from_object_raw(buf)?;
        self.object = Some(info);

        Ok(())
    }
}

impl Default for SelectTimeRange {
    fn default() -> Self {
        Self {
            begin: None,
            end: None,
        }
    }
}

impl SelectTimeRange {
    pub fn is_empty(&self) -> bool {
        self.begin.is_none() && self.end.is_none()
    }
}

impl ToString for SelectTimeRange {
    fn to_string(&self) -> String {
        if self.begin.is_some() && self.end.is_some() {
            format!(
                "{}:{}",
                self.begin.as_ref().unwrap(),
                self.end.as_ref().unwrap()
            )
        } else if self.begin.is_some() {
            format!("{}:", self.begin.as_ref().unwrap())
        } else if self.end.is_some() {
            format!(":{}", self.end.as_ref().unwrap())
        } else {
            ":".to_owned()
        }
    }
}

impl FromStr for SelectTimeRange {
    type Err = BuckyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(":").collect();
        if parts.len() != 2 {
            return Err(BuckyError::from(BuckyErrorCode::InvalidFormat));
        }

        let mut ret = Self {
            begin: None,
            end: None,
        };

        let begin = parts[0].trim();
        if !begin.is_empty() {
            let begin: u64 = begin.parse().map_err(|e| {
                error!("decode time error: {} {}", begin, e);
                BuckyError::from(BuckyErrorCode::InvalidFormat)
            })?;

            ret.begin = Some(begin);
        }

        let end = parts[1].trim();
        if !end.is_empty() {
            let end: u64 = end.parse().map_err(|e| {
                error!("decode time error: {} {}", end, e);
                BuckyError::from(BuckyErrorCode::InvalidFormat)
            })?;

            ret.end = Some(end);
        }

        Ok(ret)
    }
}

pub struct SelectResponse {
    pub objects: Vec<SelectResponseObjectInfo>,
}

impl fmt::Display for SelectResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "size:{}", self.objects.len())?;
        for item in &self.objects {
            write!(f, ",{}", item)?;
        }

        Ok(())
    }
}

impl SelectResponse {
    pub fn encode_objects(http_resp: &mut Response, objects: Vec<SelectResponseObjectInfo>) -> BuckyResult<()> {
        if objects.is_empty() {
            return Ok(());
        }

        let buf = objects.to_vec()?;
        
        debug!(
            "will send select all_buf: len={}, count={}",
            buf.len(),
            objects.len(),
            //hex::encode(&all_buf)
        );

        http_resp.set_body(buf);

        Ok(())
    }

    pub fn into_resonse(self) -> BuckyResult<Response> {
        let mut resp = RequestorHelper::new_response(StatusCode::Ok);
        if !self.objects.is_empty() {
            Self::encode_objects(&mut resp, self.objects)?;
        }

        Ok(resp)
    }

    pub async fn from_respone(mut resp: Response) -> BuckyResult<Self> {
        let all_buf = resp.body_bytes().await.map_err(|e| {
            let msg = format!("read select resp body bytes error: {}", e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        let objects = if all_buf.len() > 0 {
            let (objects, _) = Vec::raw_decode(&all_buf)?;
            objects
        } else {
            vec![]
        };
        
        debug!(
            "recv select all_buf: len={}, count={}",
            all_buf.len(),
            objects.len(),
        );

        Ok(Self { objects })
    }
}

pub struct SelectFilterUrlCodec;
impl SelectFilterUrlCodec {
    pub fn encode(url: &mut Url, filter: &SelectFilter) {
        let mut query = url.query_pairs_mut();
        if let Some(obj_type) = &filter.obj_type {
            query.append_pair("obj-type", &obj_type.to_string());
        }
        if let Some(obj_type_code) = &filter.obj_type_code {
            query.append_pair("obj-type-code", &obj_type_code.to_string());
        }

        if let Some(dec_id) = &filter.dec_id {
            query.append_pair("dec-id", &dec_id.to_string());
        }
        if let Some(owner_id) = &filter.owner_id {
            query.append_pair("owner-id", &owner_id.to_string());
        }
        if let Some(author_id) = &filter.author_id {
            query.append_pair("author-id", &author_id.to_string());
        }

        if let Some(create_time) = &filter.create_time {
            query.append_pair("create-time", &create_time.to_string());
        }
        if let Some(update_time) = &filter.update_time {
            query.append_pair("update-time", &update_time.to_string());
        }
        if let Some(insert_time) = &filter.insert_time {
            query.append_pair("insert-time", &insert_time.to_string());
        }

        if let Some(flags) = &filter.flags {
            query.append_pair("flags", &flags.to_string());
        }
    }

    pub fn decode(url: &Url) -> BuckyResult<SelectFilter> {
        let mut obj_type = None;
        let mut obj_type_code = None;

        let mut dec_id = None;
        let mut owner_id = None;
        let mut author_id = None;

        let mut create_time = None;
        let mut update_time = None;
        let mut insert_time = None;

        let mut flags = None;

        for (k, v) in url.query_pairs() {
            match k.as_ref() {
                "obj-type" => {
                    obj_type = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                "obj-type-code" => {
                    obj_type_code = Some(RequestorHelper::decode_url_param(k, v)?);
                }

                "dec-id" => {
                    dec_id = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                "owner-id" => {
                    owner_id = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                "author-id" => {
                    author_id = Some(RequestorHelper::decode_url_param(k, v)?);
                }

                "create-time" => {
                    create_time = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                "update-time" => {
                    update_time = Some(RequestorHelper::decode_url_param(k, v)?);
                }
                "insert-time" => {
                    insert_time = Some(RequestorHelper::decode_url_param(k, v)?);
                }

                "flags" => {
                    flags = Some(RequestorHelper::decode_url_param(k, v)?);
                }

                _ => {
                    warn!("unknown select filter param: {} = {}", k, v);
                }
            }
        }

        let ret = SelectFilter {
            obj_type,
            obj_type_code,

            dec_id,
            owner_id,
            author_id,

            create_time,
            insert_time,
            update_time,

            flags,
        };

        Ok(ret)
    }
}

pub struct SelectOptionCodec;

impl SelectOptionCodec {
    pub fn encode(req: &mut Request, opt: &Option<SelectOption>) {
        if let Some(opt) = opt {
            RequestorHelper::encode_header(req, cyfs_base::CYFS_PAGE_SIZE, &opt.page_size);
            RequestorHelper::encode_header(req, cyfs_base::CYFS_PAGE_INDEX, &opt.page_index);
        }
    }

    pub fn decode(req: &Request) -> BuckyResult<Option<SelectOption>> {
        let page_size: Option<u16> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_PAGE_SIZE)?;
        let page_index: Option<u16> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_PAGE_INDEX)?;

        let ret = if page_size.is_some() || page_index.is_some() {
            let mut select_opt = SelectOption::default();
            if page_size.is_some() {
                select_opt.page_size = page_size.unwrap();
            }
            if page_index.is_some() {
                select_opt.page_index = page_index.unwrap();
            }
            Some(select_opt)
        } else {
            None
        };
        Ok(ret)
    }
}

pub struct SelectEncoder {
    service_url: Url,
}

impl SelectEncoder {
    pub fn new(service_url: Url) -> Self {
        Self { service_url }
    }

    pub fn encode_select_request(&self, req: &SelectFilter, opt: Option<&SelectOption>) -> Request {
        let mut http_req = Request::new(Method::Get, self.service_url.clone());

        // TODO： 应该有通用处理
        // 允许浏览器fetch API读取私有header
        http_req.append_header("Access-Control-Allow-Headers", cyfs_base::CYFS_OBJECTS);
        http_req.append_header("Access-Control-Expose-Headers", cyfs_base::CYFS_OBJECTS);

        RequestorHelper::encode_opt_header(&mut http_req, cyfs_base::CYFS_OBJ_TYPE, &req.obj_type);
        RequestorHelper::encode_opt_header(
            &mut http_req,
            cyfs_base::CYFS_OBJ_TYPE_CODE,
            &req.obj_type_code,
        );

        RequestorHelper::encode_opt_header(
            &mut http_req,
            cyfs_base::CYFS_FILTER_DEC_ID,
            &req.dec_id,
        );
        RequestorHelper::encode_opt_header(&mut http_req, cyfs_base::CYFS_OWNER_ID, &req.owner_id);
        RequestorHelper::encode_opt_header(
            &mut http_req,
            cyfs_base::CYFS_AUTHOR_ID,
            &req.author_id,
        );

        RequestorHelper::encode_opt_header(
            &mut http_req,
            cyfs_base::CYFS_CREATE_TIME,
            &req.create_time,
        );
        RequestorHelper::encode_opt_header(
            &mut http_req,
            cyfs_base::CYFS_UPDATE_TIME,
            &req.update_time,
        );
        RequestorHelper::encode_opt_header(
            &mut http_req,
            cyfs_base::CYFS_INSERT_TIME,
            &req.insert_time,
        );

        RequestorHelper::encode_opt_header(&mut http_req, cyfs_base::CYFS_FILTER_FLAGS, &req.flags);

        if opt.is_some() {
            let opt = opt.unwrap();

            RequestorHelper::encode_header(
                &mut http_req,
                cyfs_base::CYFS_PAGE_SIZE,
                &opt.page_size,
            );
            RequestorHelper::encode_header(
                &mut http_req,
                cyfs_base::CYFS_PAGE_INDEX,
                &opt.page_index,
            );
        }

        http_req
    }
}

pub struct SelectDecoder;

impl SelectDecoder {
    pub fn decode_select_request(req: &Request) -> BuckyResult<(SelectFilter, SelectOption)> {
        // SelectFilter
        let obj_type: Option<u16> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_OBJ_TYPE)?;
        let obj_type_code: Option<ObjectTypeCode> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_OBJ_TYPE_CODE)?;

        let dec_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_FILTER_DEC_ID)?;
        let owner_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_OWNER_ID)?;
        let author_id: Option<ObjectId> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_AUTHOR_ID)?;

        let create_time: Option<SelectTimeRange> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_CREATE_TIME)?;
        let update_time: Option<SelectTimeRange> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_UPDATE_TIME)?;
        let insert_time: Option<SelectTimeRange> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_INSERT_TIME)?;

        let flags: Option<u32> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_FILTER_FLAGS)?;

        // SelectOption
        let page_size: Option<u16> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_PAGE_SIZE)?;
        let page_index: Option<u16> =
            RequestorHelper::decode_optional_header(req, cyfs_base::CYFS_PAGE_INDEX)?;

        let select_req = SelectFilter {
            obj_type,
            obj_type_code,

            dec_id,
            owner_id,
            author_id,

            create_time,
            update_time,
            insert_time,

            flags,
        };

        let mut select_opt = SelectOption::default();
        if page_size.is_some() {
            select_opt.page_size = page_size.unwrap();
        }
        if page_index.is_some() {
            select_opt.page_index = page_index.unwrap();
        }

        Ok((select_req, select_opt))
    }
}

impl JsonCodec<SelectTimeRange> for SelectTimeRange {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_number_field(&mut obj, "begin", self.begin);
        JsonCodecHelper::encode_option_number_field(&mut obj, "end", self.end);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<SelectTimeRange> {
        Ok(Self {
            begin: JsonCodecHelper::decode_option_int_field(obj, "begin")?,
            end: JsonCodecHelper::decode_option_int_field(obj, "end")?,
        })
    }
}

impl JsonCodec<SelectFilter> for SelectFilter {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_option_number_field(&mut obj, "obj_type", self.obj_type);
        JsonCodecHelper::encode_option_number_field(
            &mut obj,
            "obj_type_code",
            self.obj_type_code.as_ref().map(|v| v.to_u16()),
        );

        JsonCodecHelper::encode_option_string_field(&mut obj, "dec_id", self.dec_id.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "owner_id", self.owner_id.as_ref());
        JsonCodecHelper::encode_option_string_field(&mut obj, "author_id", self.author_id.as_ref());

        JsonCodecHelper::encode_option_field(&mut obj, "create_time", self.create_time.as_ref());
        JsonCodecHelper::encode_option_field(&mut obj, "update_time", self.update_time.as_ref());
        JsonCodecHelper::encode_option_field(&mut obj, "insert_time", self.insert_time.as_ref());

        JsonCodecHelper::encode_option_number_field(&mut obj, "flags", self.flags);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<SelectFilter> {
        let obj_type_code: Option<u16> =
            JsonCodecHelper::decode_option_int_field(obj, "obj_type_code")?;
        Ok(Self {
            obj_type: JsonCodecHelper::decode_option_int_field(obj, "obj_type")?,
            obj_type_code: obj_type_code.map(|v| v.into()),

            dec_id: JsonCodecHelper::decode_option_string_field(obj, "dec_id")?,
            owner_id: JsonCodecHelper::decode_option_string_field(obj, "owner_id")?,
            author_id: JsonCodecHelper::decode_option_string_field(obj, "author_id")?,

            create_time: JsonCodecHelper::decode_option_field(obj, "create_time")?,
            update_time: JsonCodecHelper::decode_option_field(obj, "update_time")?,
            insert_time: JsonCodecHelper::decode_option_field(obj, "insert_time")?,

            flags: JsonCodecHelper::decode_option_int_field(obj, "flags")?,
        })
    }
}

impl JsonCodec<SelectOption> for SelectOption {
    fn encode_json(&self) -> Map<String, Value> {
        let mut obj = Map::new();

        JsonCodecHelper::encode_number_field(&mut obj, "page_index", self.page_index);
        JsonCodecHelper::encode_number_field(&mut obj, "page_size", self.page_size);

        obj
    }

    fn decode_json(obj: &Map<String, Value>) -> BuckyResult<SelectOption> {
        Ok(Self {
            page_index: JsonCodecHelper::decode_int_field(obj, "page_index")?,
            page_size: JsonCodecHelper::decode_int_field(obj, "page_size")?,
        })
    }
}

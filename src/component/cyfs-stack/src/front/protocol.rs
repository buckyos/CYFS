use super::def::*;
use super::http_request::FrontInputHttpRequest;
use super::listener::FrontRequestType;
use super::request::*;
use super::service::*;
use crate::name::*;
use crate::ndn_api::NDNRequestHandler;
use crate::non_api::NONRequestHandler;
use crate::zone::ZoneManagerRef;
use cyfs_base::*;
use cyfs_lib::*;

use std::str::FromStr;
use std::sync::Arc;

// object_id base58编码后的长度范围
const PATH_SEGMENT_OBJECT_ID_MIN_LEN: usize = 42;
const PATH_SEGMENT_OBJECT_ID_MAX_LEN: usize = 45;

const KNOWN_ROOTS: &[&str] = &[
    "handler",
    "non",
    "ndn",
    "crypto",
    "util",
    "sync",
    "trans",
    "root-state",
    "local-cache",
    "system",
    "root",
    "o",
    "r",
    "l",
    "a",
];

pub(crate) fn parse_front_host_with_dec_id(
    host: &str,
) -> BuckyResult<Option<(FrontRequestType, ObjectId)>> {
    let ft = if host.starts_with("o.") {
        FrontRequestType::O
    } else if host.starts_with("a.") {
        FrontRequestType::A
    } else if host.starts_with("r.") {
        FrontRequestType::R
    } else if host.starts_with("l.") {
        FrontRequestType::L
    } else {
        return Ok(None);
    };

    let s = &host[2..];
    match s {
        "system" => {
            Ok(Some((ft, cyfs_core::get_system_dec_app().to_owned())))
        }
        _ => {
            match ObjectId::from_str(s) {
                Ok(dec_id) => Ok(Some((ft, dec_id))),
                Err(e) => {
                    let msg = format!("invalid front host's dec_id! host={}, {}", host, e);
                    warn!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
                }
            }
        }
    }
}

fn is_host_object_id(host: &str) -> bool {
    if OBJECT_ID_BASE36_RANGE.contains(&host.len()) || OBJECT_ID_BASE58_RANGE.contains(&host.len())
    {
        match ObjectId::from_str(host) {
            Ok(_) => true,
            Err(_) => false,
        }
    } else {
        false
    }
}

pub(crate) fn parse_front_host(host: &str) -> Option<FrontRequestType> {
    let ft = match host {
        "o" => FrontRequestType::O,
        "a" => FrontRequestType::A,
        "r" => FrontRequestType::R,
        "l" => FrontRequestType::L,
        _ => {
            if host.starts_with("o.") {
                FrontRequestType::O
            } else if host.starts_with("a.") {
                FrontRequestType::A
            } else if host.starts_with("r.") {
                FrontRequestType::R
            } else if host.starts_with("l.") {
                FrontRequestType::L
            } else {
                if is_host_object_id(host) {
                    FrontRequestType::O
                } else {
                    // treat as name
                    FrontRequestType::O
                }
            }
        }
    };

    Some(ft)
}

pub(crate) fn parse_front_host_with_anonymous_dec_id(
    host: &str,
) -> Option<(FrontRequestType, ObjectId)> {
    let ft = match host {
        "o" => FrontRequestType::O,
        "a" => FrontRequestType::A,
        "r" => FrontRequestType::R,
        "l" => FrontRequestType::L,
        _ => {
            if is_host_object_id(host) {
                FrontRequestType::O
            } else {
                // treat as name
                FrontRequestType::O
            }
        }
    };

    Some((ft, cyfs_core::get_anonymous_dec_app().to_owned()))
}

pub(crate) struct FrontProtocolHandler {
    name_resolver: NameResolver,
    zone_manager: ZoneManagerRef,
    service: Arc<FrontService>,
}

pub(crate) type FrontProtocolHandlerRef = Arc<FrontProtocolHandler>;

impl FrontProtocolHandler {
    pub fn new(
        name_resolver: NameResolver,
        zone_manager: ZoneManagerRef,
        service: Arc<FrontService>,
    ) -> Self {
        Self {
            name_resolver,
            zone_manager,
            service,
        }
    }

    fn extract_route_param<State>(req: &tide::Request<State>) -> BuckyResult<String> {
        match Self::extract_option_route_param(req)? {
            Some(value) => Ok(value),
            None => {
                let msg = format!("request url must param missing! {}", req.url());
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    fn extract_option_route_param<State>(
        req: &tide::Request<State>,
    ) -> BuckyResult<Option<String>> {
        match req.param("must") {
            Ok(v) => {
                // 对url里面的以%编码的unicode字符进行解码
                let decoded_value = percent_encoding::percent_decode_str(&v);
                let value = decoded_value.decode_utf8().map_err(|e| {
                    let msg = format!("invalid utf8 url format! param={}, {}", v, e);
                    error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
                })?;

                Ok(Some(value.into_owned()))
            }
            Err(_) => Ok(None),
        }
    }

    // 直接解析一个seg是不是object_id
    fn parse_object_seg(seg: &str) -> Option<ObjectId> {
        // 只对合适的字符串才尝试解析是不是object
        // TODO 进一步优化
        if seg.len() >= PATH_SEGMENT_OBJECT_ID_MIN_LEN
            && seg.len() <= PATH_SEGMENT_OBJECT_ID_MAX_LEN
        {
            match ObjectId::from_str(seg) {
                Ok(id) => Some(id),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    // 解析seg列表，seg存在下面两种情况
    // 1. 编码后的object_id
    // 2. object_id对应的name
    async fn resolve_segs(&self, seg: &str) -> BuckyResult<Vec<ObjectId>> {
        let mut result = Vec::new();

        // multi part for zone's seg is valid
        let items: Vec<&str> = seg.split(',').collect();
        for item in items {
            // CYFS_NAME_MAX_LENGTH为边界，大于此长度则是object_id，否则认为是name
            if item.len() > CYFS_NAME_MAX_LENGTH {
                match ObjectId::from_str(item) {
                    Ok(id) => {
                        result.push(id);
                    }
                    Err(e) => {
                        let msg = format!("invalid url seg as object_id: {}, {}", item, e);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                };
            } else {
                // 解析name
                let id = self.lookup_name(item).await.map_err(|e| {
                    error!("resolve as name but failed! seg={}, {}", item, e);
                    e
                })?;

                info!("name resolved: {} -> {}", item, id);
                result.push(id);
            }
        }

        Ok(result)
    }

    // 解析name<->object的绑定关系
    async fn lookup_name(&self, name: &str) -> BuckyResult<ObjectId> {
        match self.name_resolver.lookup(name).await? {
            NameResult::ObjectLink(id) => Ok(id),
            NameResult::IPLink(addr) => {
                let msg = format!("name system not support iplink yet! {} -> {}", name, addr);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }

    fn mode_from_request(url: &http_types::Url) -> BuckyResult<FrontRequestGetMode> {
        match RequestorHelper::value_from_querys("mode", url) {
            Ok(Some(mode)) => Ok(mode),
            Ok(None) => Ok(FrontRequestGetMode::Default),
            Err(e) => {
                let msg = format!("invalid request url mode query param! {}, {}", url, e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    fn object_format_from_request(url: &http_types::Url) -> BuckyResult<FrontRequestObjectFormat> {
        match RequestorHelper::value_from_querys("format", url) {
            Ok(Some(format)) => Ok(format),
            Ok(None) => Ok(FrontRequestObjectFormat::Default),
            Err(e) => {
                let msg = format!("invalid request url format query param! {}, {}", url, e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    fn range_from_request(req: &http_types::Request) -> BuckyResult<Option<NDNDataRequestRange>> {
        // first extract dec_id from headers
        let s: Option<String> = match RequestorHelper::decode_optional_header(req, "Range")? {
            Some(range) => Some(range),
            None => {
                // try extract range from query pairs
                match RequestorHelper::value_from_querys("range", req.url()) {
                    Ok(v) => v,
                    Err(e) => {
                        let msg = format!(
                            "invalid request url range query param! {}, {}",
                            req.url(),
                            e
                        );
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
                    }
                }
            }
        };

        Ok(s.map(|s| NDNDataRequestRange::new_unparsed(s)))
    }

    fn flags_from_request(url: &http_types::Url) -> BuckyResult<u32> {
        // try extract flags from query pairs
        match RequestorHelper::value_from_querys("flags", url) {
            Ok(Some(v)) => Ok(v),
            Ok(None) => Ok(0),
            Err(e) => {
                let msg = format!("invalid request url flags query param! {}, {}", url, e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    fn referer_objects_from_request(
        url: &http_types::Url,
    ) -> BuckyResult<Vec<NDNDataRefererObject>> {
        // try extract referer from query pairs
        match RequestorHelper::value_from_querys_with_utf8_decoding("referer", url) {
            Ok(Some(v)) => Ok(vec![v]),
            Ok(None) => Ok(vec![]),
            Err(e) => {
                let msg = format!("invalid request url referer query param! {}, {}", url, e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    fn context_from_request(url: &http_types::Url) -> BuckyResult<Option<String>> {
        // try extract group from query pairs
        match RequestorHelper::value_from_querys_with_utf8_decoding("context", url) {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) => Ok(None),
            Err(e) => {
                let msg = format!("invalid request url context query param! {}, {}", url, e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    fn group_from_request(url: &http_types::Url) -> BuckyResult<Option<String>> {
        // try extract group from query pairs
        match RequestorHelper::value_from_querys_with_utf8_decoding("group", url) {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) => Ok(None),
            Err(e) => {
                let msg = format!("invalid request url group query param! {}, {}", url, e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    fn req_path_from_request(
        url: &http_types::Url,
    ) -> BuckyResult<Option<String>> {
        // try extract referer from query pairs
        match RequestorHelper::value_from_querys_with_utf8_decoding("req_path", url) {
            Ok(ret) => Ok(ret),
            Err(e) => {
                let msg = format!("invalid request url req_path query param! {}, {}", url, e);
                error!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg))
            }
        }
    }

    fn is_cyfs_browser(req: &http_types::Request) -> bool {
        let ret: BuckyResult<Option<String>> =
            RequestorHelper::decode_optional_header(req, http_types::headers::USER_AGENT);
        match ret {
            Ok(Some(s)) => {
                if s.as_str().contains("CYFS Browser") {
                    true
                } else {
                    false
                }
            }
            Ok(None) => false,
            Err(e) => {
                warn!("decode user-agent from http request header error! {}", e);
                false
            }
        }
    }

    fn parse_url_segs(route_param: &str) -> BuckyResult<Vec<&str>> {
        let segs: Vec<&str> = route_param
            .trim_start_matches('/')
            .split('/')
            .filter(|seg| !seg.is_empty())
            .collect();
        if segs.is_empty() {
            let msg = format!("invalid request url param! param={}", route_param);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        Ok(segs)
    }

    fn gen_inner_path(segs: &[&str]) -> String {
        let path = segs.join("/");
        format!("/{}", path)
    }

    pub async fn process_request<State>(
        &self,
        req_type: FrontRequestType,
        req: FrontInputHttpRequest<State>,
    ) -> tide::Response {
        match self.process_request_inner(req_type, req).await {
            Ok(resp) => resp,
            Err(e) => RequestorHelper::trans_error(e),
        }
    }

    async fn process_request_inner<State>(
        &self,
        req_type: FrontRequestType,
        req: FrontInputHttpRequest<State>,
    ) -> BuckyResult<tide::Response> {
        let format = Self::object_format_from_request(req.request.url())?;

        match req_type {
            FrontRequestType::O => {
                let route_param = Self::extract_route_param(&req.request)?;
                let resp = self.process_o_request(req, route_param, format).await?;

                let http_resp = self.encode_o_response(resp, format).await;
                Ok(http_resp)
            }
            FrontRequestType::R | FrontRequestType::L => {
                let route_param = Self::extract_route_param(&req.request)?;
                let resp = self.process_r_request(req_type, req, route_param).await?;

                let http_resp = self.encode_r_response(resp, format).await;
                Ok(http_resp)
            }
            FrontRequestType::A => {
                let route_param = Self::extract_route_param(&req.request)?;
                let is_cyfs_browser = Self::is_cyfs_browser(&req.request.as_ref());
                let resp = self.process_a_request(req, route_param, format).await?;

                let http_resp = self.encode_a_response(resp, format, is_cyfs_browser).await;
                Ok(http_resp)
            }
            FrontRequestType::Any => {
                let route_param = Self::extract_option_route_param(&req.request)?;
                self.process_any_request(req, route_param, format).await
            }
        }
    }

    async fn process_any_request<State>(
        &self,
        mut req: FrontInputHttpRequest<State>,
        route_param: Option<String>,
        format: FrontRequestObjectFormat,
    ) -> BuckyResult<tide::Response> {
        let name = req.request.param("name").map_err(|e| {
            let msg = format!(
                "invalid request url root param! {}, {}",
                req.request.url(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidParam, msg)
        })?;

        let req_type;
        let req_route_param;
        if let Some((ft, dec_id)) = parse_front_host_with_dec_id(name)? {
            // check dec if matched
            if let Some(dec) = req.source.get_opt_dec() {
                if *dec != dec_id {
                    let msg = format!(
                        "request source id not matched! url={}, req dec={}",
                        req.request.url(),
                        dec,
                    );
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                }
            } else {
                warn!(
                    "request source dec missing! now will set as host.dec_id! url={}",
                    req.request.url()
                );
                req.source.set_dec(dec_id);
            }

            if route_param.is_none() {
                let msg = format!("request url route param missing! {}", req.request.url(),);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }

            req_type = ft;
            req_route_param = route_param.unwrap();
        } else if KNOWN_ROOTS.iter().find(|v| **v == name).is_some() {
            let msg = format!(
                "reserved request url root param! {}, root={}",
                req.request.url(),
                name
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        } else {
            // treat as o request default
            req_type = FrontRequestType::O;
            req_route_param = match route_param {
                Some(param) => format!("{}/{}", name, param),
                None => name.to_owned(),
            };
        }

        match req_type {
            FrontRequestType::O => {
                let resp = self.process_o_request(req, req_route_param, format).await?;
                let http_resp = self.encode_o_response(resp, format).await;
                Ok(http_resp)
            }
            FrontRequestType::A => {
                let is_cyfs_browser = Self::is_cyfs_browser(&req.request.as_ref());
                let resp = self.process_a_request(req, req_route_param, format).await?;
                let http_resp = self.encode_a_response(resp, format, is_cyfs_browser).await;
                Ok(http_resp)
            }
            FrontRequestType::R | FrontRequestType::L => {
                let resp = self
                    .process_r_request(req_type, req, req_route_param)
                    .await?;
                let http_resp = self.encode_r_response(resp, format).await;
                Ok(http_resp)
            }
            FrontRequestType::Any => {
                unreachable!()
            }
        }
    }

    async fn process_o_request<State>(
        &self,
        req: FrontInputHttpRequest<State>,
        route_param: String,
        format: FrontRequestObjectFormat,
    ) -> BuckyResult<FrontOResponse> {
        let segs = Self::parse_url_segs(&route_param)?;
        let url = req.request.url();

        assert!(segs.len() > 0);
        let root = segs[0];
        if root.is_empty() {
            let msg = format!("invalid request url root param! {}", url,);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let mode = Self::mode_from_request(url)?;
        let flags = Self::flags_from_request(url)?;

        let range = Self::range_from_request(req.request.as_ref())?;

        let referer_objects = Self::referer_objects_from_request(&url)?;
        let context = Self::context_from_request(&url)?;
        let group = Self::group_from_request(&url)?;
        let req_path = Self::req_path_from_request(&url)?;

        /*
        /object_id
        /object_id/inner_path
        /owner_id/object_id
        /owner_id/object_id/inner_path
        */

        // first segment must be valid object_id
        let roots = self.resolve_segs(root).await?;

        // second segment can be object_id, or inner_path's seg
        let second_seg = if segs.len() >= 2 {
            Self::parse_object_seg(segs[1])
        } else {
            None
        };

        let o_req = match second_seg {
            Some(id) => {
                // treat as two seg mode

                let inner_path = if segs.len() > 2 {
                    Some(Self::gen_inner_path(&segs[2..]))
                } else {
                    None
                };

                FrontORequest {
                    source: req.source,

                    req_path,
                    target: roots,

                    object_id: id,
                    inner_path,
                    range,

                    mode,
                    format,

                    referer_objects,
                    context,
                    group,

                    flags,
                }
            }
            None => {
                // treat as one seg mode, only single object is accepted
                if roots.len() != 1 {
                    let msg = format!("only single root path support: {}", root);
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                let inner_path = if segs.len() > 1 {
                    Some(Self::gen_inner_path(&segs[1..]))
                } else {
                    None
                };

                FrontORequest {
                    source: req.source,
                    
                    req_path,
                    target: vec![],

                    object_id: roots[0],
                    inner_path,
                    range,

                    mode,
                    format,

                    referer_objects,
                    context,
                    group,

                    flags,
                }
            }
        };

        self.service.process_o_request(o_req).await
    }

    fn parse_dec_seg(
        url: &http_types::Url,
        segs: &Vec<&str>,
        pos: usize,
    ) -> BuckyResult<Option<ObjectId>> {
        if segs.len() <= pos {
            let msg = format!("invalid request url dec_id seg! {}", url,);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let dec_seg = segs[pos];
        match dec_seg {
            "system" => Ok(Some(cyfs_core::get_system_dec_app().to_owned())),
            "root" => Ok(None),
            _ => {
                match Self::parse_object_seg(dec_seg) {
                    Some(id) => {
                        match id.obj_type_code() {
                            ObjectTypeCode::Custom => {
                                // treat as dec_id
                                Ok(Some(id))
                            }
                            code @ _ => {
                                let msg = format!(
                                    "invalid r path dec seg type: {}, type_code={:?}",
                                    dec_seg, code
                                );
                                error!("{}", msg);
                                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
                            }
                        }
                    }
                    None => {
                        let msg = format!("invalid r path dec seg: {}", dec_seg);
                        error!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
                    }
                }
            }
        }
    }

    async fn process_r_request<State>(
        &self,
        req_type: FrontRequestType,
        req: FrontInputHttpRequest<State>,
        route_param: String,
    ) -> BuckyResult<FrontRResponse> {
        /*
        [/target]/{dec_id}/{inner_path}

        target: People/SimpleGroup/Device-id, name, $, $$
        dec-id: DecAppId/system/root
        */

        let segs = Self::parse_url_segs(&route_param)?;
        let url = req.request.url();

        assert!(segs.len() > 0);
        let root = segs[0];
        if root.is_empty() {
            let msg = format!("invalid request url root param! {}", url,);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let target;
        let target_dec_id;
        let inner_path_pos;
        match root {
            "$" => {
                // treat as two seg mode
                target = None;
                target_dec_id = Self::parse_dec_seg(url, &segs, 1)?;
                inner_path_pos = 2;
            }
            "$$" => {
                // treat as two seg mode
                let ood_id = self
                    .zone_manager
                    .get_current_info()
                    .await?
                    .zone_device_ood_id
                    .object_id()
                    .clone();
                target = Some(ood_id);
                target_dec_id = Self::parse_dec_seg(url, &segs, 1)?;
                inner_path_pos = 2;
            }
            "system" => {
                // treat as one seg mode
                target = None;
                target_dec_id = Some(cyfs_core::get_system_dec_app().to_owned());
                inner_path_pos = 1;
            }
            "root" => {
                // treat as one seg mode
                target = None;
                target_dec_id = None;
                inner_path_pos = 1;
            }
            _ => {
                // parse first segs, then check the objectid's type code to decide which mode
                let seg_objects = self.resolve_segs(root).await?;
                if seg_objects.len() != 1 {
                    let msg = format!("only single target path support: {}", root);
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                let seg_object = seg_objects[0];
                match seg_object.obj_type_code() {
                    ObjectTypeCode::Device
                    | ObjectTypeCode::People
                    | ObjectTypeCode::SimpleGroup => {
                        // treat as two seg mode
                        target = Some(seg_object);
                        target_dec_id = Self::parse_dec_seg(url, &segs, 1)?;
                        inner_path_pos = 2;
                    }
                    ObjectTypeCode::Custom => {
                        // treat as one seg mode
                        target = None;
                        target_dec_id = Some(seg_object);
                        inner_path_pos = 1;
                    }
                    _ => {
                        let msg = format!(
                            "invalid r path target|dec seg type: {}, type_code={:?}",
                            seg_object,
                            seg_object.obj_type_code()
                        );
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                }
            }
        }

        let inner_path = if segs.len() >= inner_path_pos {
            Some(Self::gen_inner_path(&segs[inner_path_pos..]))
        } else {
            None
        };

        let category = match req_type {
            FrontRequestType::R => GlobalStateCategory::RootState,
            FrontRequestType::L => GlobalStateCategory::LocalCache,
            _ => unreachable!(),
        };

        let range = Self::range_from_request(req.request.as_ref())?;

        // let mode = Self::mode_from_request(url)?;
        // let flags = Self::flags_from_request(url)?;

        // extract params from url querys
        let mut page_index: Option<u32> = None;
        let mut page_size: Option<u32> = None;
        let mut action = GlobalStateAccessorAction::GetObjectByPath;
        let mut mode = FrontRequestGetMode::Default;
        let mut flags = 0;
        let mut context = None;
        let mut group = None;

        let pairs = req.request.url().query_pairs();
        let mut user_pairs = vec![];
        for (k, v) in pairs {
            match k.as_ref() {
                "mode" => {
                    mode = FrontRequestGetMode::from_str(v.as_ref())?;
                }
                "format" => { /* ignore */ }
                "flags" => {
                    flags = u32::from_str(v.as_ref()).map_err(|e| {
                        let msg = format!(
                            "invalid request url flags query param! {}, {}",
                            req.request.url(),
                            e
                        );
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::InvalidParam, msg)
                    })?;
                }
                "action" => {
                    action = GlobalStateAccessorAction::from_str(v.as_ref())?;
                }
                "page_index" => {
                    let v = v.as_ref().parse().map_err(|e| {
                        let msg = format!("invalid page_index param: {}, {}", v, e);
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::InvalidParam, msg)
                    })?;
                    page_index = Some(v);
                }
                "page_size" => {
                    let v = v.as_ref().parse().map_err(|e| {
                        let msg = format!("invalid page_size param: {}, {}", v, e);
                        error!("{}", msg);
                        BuckyError::new(BuckyErrorCode::InvalidParam, msg)
                    })?;
                    page_size = Some(v);
                }
                "context" => {
                    context = Some(RequestorHelper::decode_url_param_with_utf8_decoding(k, v)?);
                }
                "group" => {
                    group = Some(RequestorHelper::decode_url_param_with_utf8_decoding(k, v)?);
                }
                _ => {
                    debug!("user global state access url query: {}={}", k, v);
                    user_pairs.push(format!("{}={}", k, v));
                }
            }
        }

        let inner_path: Option<String> = if let Some(inner_path) = inner_path {
            if user_pairs.is_empty() {
                Some(inner_path)
            } else {
                let user_querys = user_pairs.join("&");
                Some(format!("{}?{}", inner_path, user_querys))
            }
        } else {
            None
        };

        
        let r_req = FrontRRequest {
            source: req.source,

            category,

            target,
            target_dec_id,

            action,
            inner_path,
            range,
            page_index,
            page_size,

            mode,
            context,
            group,

            flags,
        };

        self.service.process_r_request(r_req).await
    }

    /*
    cyfs://a/{dec-id}/{inner-path}
    cyfs://a/{dec-id}/{dir-id}/{inner-path}
    cyfs://a/{dec-id}/{x.x.x}/{inner-path}
    cyfs://a/{dec-id}/local_status
    */
    async fn process_a_request<State>(
        &self,
        req: FrontInputHttpRequest<State>,
        route_param: String,
        format: FrontRequestObjectFormat,
    ) -> BuckyResult<FrontAResponse> {
        let segs = Self::parse_url_segs(&route_param)?;
        let url = req.request.url();

        assert!(segs.len() > 0);
        if segs.len() < 2 {
            let msg = format!("invalid request url root segs! {}", url,);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }
        let dec = match Self::parse_object_seg(segs[0]) {
            Some(id) => FrontARequestDec::DecID(id),
            None => FrontARequestDec::Name(segs[0].to_owned()),
        };

        let goal = match segs[1] {
            "local_status" => FrontARequestGoal::LocalStatus,
            _ => {
                let mut inner_path_pos = 2;
                let version = match Self::parse_object_seg(segs[1]) {
                    Some(id) => FrontARequestVersion::DirID(id),
                    None => {
                        // check if semversion
                        match semver::Version::parse(segs[1]) {
                            Ok(_version) => FrontARequestVersion::Version(segs[1].to_owned()),
                            Err(_) => {
                                inner_path_pos = 1;
                                FrontARequestVersion::Current
                            }
                        }
                    }
                };

                let inner_path = if segs.len() > inner_path_pos {
                    Some(Self::gen_inner_path(&segs[inner_path_pos..]))
                } else {
                    None
                };

                let web_req = FrontARequestWeb {
                    version,
                    inner_path,
                };

                FrontARequestGoal::Web(web_req)
            }
        };

        let mode = Self::mode_from_request(url)?;
        let flags = Self::flags_from_request(url)?;
        let referer_objects = Self::referer_objects_from_request(url)?;
        let context = Self::context_from_request(url)?;
        let group = Self::group_from_request(url)?;

        // TODO now target always be current zone's ood
        let target = self
            .zone_manager
            .get_current_info()
            .await?
            .zone_device_ood_id
            .clone();

        let mut flush_cache = false;
        if let Some(cc) = req.request.header(http_types::headers::CACHE_CONTROL) {
            if let Some(cache_control) = cache_control::CacheControl::from_value(cc.last().as_str()) {
                if cache_control.no_store || cache_control.must_revalidate {
                    flush_cache = true;
                } else if cache_control.cachability == Some(cache_control::Cachability::NoCache) {
                    flush_cache = true;
                }
            }
        }

        let a_req = FrontARequest {
            source: req.source,
            target: Some(target.into()),

            dec,
            goal,

            mode,
            format,

            origin_url: url.to_owned(),

            referer_objects,
            context,
            group,

            flags,

            flush_cache,
        };

        self.service.process_a_request(a_req).await
    }

    async fn encode_o_response(
        &self,
        resp: FrontOResponse,
        format: FrontRequestObjectFormat,
    ) -> tide::Response {
        match resp.data {
            Some(data_resp) => {
                let mut http_resp = NDNRequestHandler::encode_get_data_response(data_resp);

                if let Some(object_resp) = resp.object {
                    NONRequestHandler::encode_get_object_response_times(
                        http_resp.as_mut(),
                        &object_resp,
                    );
                }

                http_resp
            }
            None => {
                let object_resp = resp.object.unwrap();
                NONRequestHandler::encode_get_object_response(object_resp, format)
            }
        }
    }

    async fn encode_r_response(
        &self,
        resp: FrontRResponse,
        format: FrontRequestObjectFormat,
    ) -> tide::Response {
        let mut http_resp = if let Some(data_resp) = resp.data {
            let mut http_resp = NDNRequestHandler::encode_get_data_response(data_resp);

            if let Some(object_resp) = resp.object {
                NONRequestHandler::encode_get_object_response_times(
                    http_resp.as_mut(),
                    &object_resp,
                );
            }

            http_resp
        } else if let Some(object_resp) = resp.object {
            NONRequestHandler::encode_get_object_response(object_resp, format)
        } else if let Some(list_resp) = resp.list {
            let mut http_resp = RequestorHelper::new_response(http_types::StatusCode::Ok);
            http_resp.set_body(list_resp.encode_string());
            http_resp.set_content_type(tide::http::mime::JSON);
            http_resp.into()
        } else {
            unreachable!();
        };

        http_resp.insert_header(cyfs_base::CYFS_ROOT, resp.root.to_string());
        http_resp.insert_header(cyfs_base::CYFS_REVISION, resp.revision.to_string());

        http_resp
    }

    async fn encode_a_response(
        &self,
        resp: FrontAResponse,
        format: FrontRequestObjectFormat,
        is_cyfs_browser: bool,
    ) -> tide::Response {
        match resp {
            FrontAResponse::Response(o_resp) => self.encode_o_response(o_resp, format).await,
            FrontAResponse::Redirect(mut url) => {
                if is_cyfs_browser {
                    url = format!("cyfs:/{}", url);
                }
                tide::Redirect::new(url).into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_o_query_string() {
        let url = "http://www.cyfs.com/a/b?req_path=/a/b%3Ftoken=xxx%26id=xxx&dec_id=xxx";
        let url = http_types::Url::parse(url).unwrap();
        let value: String = RequestorHelper::value_from_querys_with_utf8_decoding("dec_id", &url).unwrap().unwrap();
        assert_eq!(value, "xxx");

        let value: String = RequestorHelper::value_from_querys_with_utf8_decoding("req_path", &url).unwrap().unwrap();
        assert_eq!(value, "/a/b?token=xxx&id=xxx");
    }
}
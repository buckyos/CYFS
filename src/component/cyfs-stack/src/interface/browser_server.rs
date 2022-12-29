use cyfs_base::*;
use super::http_server::*;
use cyfs_lib::*;

use std::sync::Arc;

/*
The browser only accepts requests from the following two types of pages:
cyfs://static
It is considered to be a management page, with high authority (system-dec-id), no source verification, and can simulate as any dec-id

cyfs://a|o|r.{dec-id}
If it is considered to be the specified app page, it is necessary to verify whether the source-dec-id matches the dec-id in the reference url. If it does not match, an error will be returned.

other pages
In strict mode, requests for unknown pages are not accepted; in loose mode, anonymous dec-id is uniformly used for unknown pages
*/
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
enum RequestSource {
    System,
    Dec(ObjectId),
    Extension,
    Other,
}

#[derive(Debug)]
enum RequestSourceString<'a> {
    Origin(&'a str),
    Host(&'a str),
    Extension,
    Other,
}

pub(super) struct BrowserSanboxHttpServer {
    mode: BrowserSanboxMode,
    handler: HttpServerHandlerRef,
}

impl BrowserSanboxHttpServer {
    pub(crate) fn new(
        handler: HttpServerHandlerRef,
        mode: BrowserSanboxMode,
    ) -> Self {
        assert_ne!(mode, BrowserSanboxMode::None);

        Self {
            handler,
            mode,
        }
    }

    pub fn into(self) -> HttpServerHandlerRef {
        Arc::new(Box::new(self))
    }

    /*
    cyfs://static
    cyfs://o.{dec_id}/
    cyfs://o/
    cyfs://{object-id}
    cyfs://{name}
    */
    fn parse_host(host: &str) -> BuckyResult<RequestSource> {
        if host == "static" {
            return Ok(RequestSource::System);
        }
        
        // Parse host in a|o|r|l.dec_id mode
        if let Some((_, dec_id)) = crate::front::parse_front_host_with_dec_id(host)? {
            return Ok(RequestSource::Dec(dec_id));
        }

        // Parse host in raw a|o|r|l|{object-id}|{name} mode, treat as anonymous dec_id
        if let Some((_, dec_id)) = crate::front::parse_front_host_with_anonymous_dec_id(host) {
            return Ok(RequestSource::Dec(dec_id));
        }

        warn!("unknown request origin/referer host! host={}", host);
        Ok(RequestSource::Other)
    }

    // http://127.0.0.1:xxx/a|o|r|l[.dec_id]/xxx -> a|o|r|l[.dec_id]
    // http://127.0.0.1:xxx/{object_id}|{name} -> {object_id}
    fn extract_front_root<'a>(req: &'a http_types::Request,) -> Option<&'a str> {
        let mut ret = req.url().path().trim_start_matches('/').split('/');
        let host = ret.next();
        if host.is_none() {
            return None;
        }

        let host = host.unwrap();
        if crate::front::parse_front_host(host).is_none() {
            return None;
        }

        Some(host)
    }

    fn check_extension_request(req: &http_types::Request) -> BuckyResult<()> {
        let ret: Option<ObjectId> = RequestorHelper::dec_id_from_request(&req)?;
        match ret {
            Some(source_dec_id) => {
                if source_dec_id == *cyfs_core::get_system_dec_app() {
                    let msg = format!("request from browser extensions's dec_id cannot be specified as system-dec-id! req={}", 
                        req.url(), 
                    );

                    warn!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
                } else {
                    // debug!("request from browser extensions: {}", req.url());
                    Ok(())
                }
            }
            None => {
                let msg = format!("request from browser extensions but dec_id header or query pairs missing! req={}", req.url());
                warn!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
            }
        }
    }

    // FIXME 目前chrome插件的请求，不确定的原因导致部分请求不带origin，所以会进入到此处理Other分支
    // 所以我们这里先判断下，如果提供了cyfs-dec-id的header，那么认为是插件sdk发起的请求，只需要校验身份不能是system-dec-id
    fn check_other_request<'a>(req: &http_types::Request) -> BuckyResult<Option<RequestSourceString<'a>>> {
        let ret: Option<ObjectId> = RequestorHelper::dec_id_from_request(&req)?;
        match ret {
            Some(source_dec_id) => {
                // FIXME 暂时认为是插件发起的请求
                if source_dec_id == *cyfs_core::get_system_dec_app() {
                    let msg = format!("request from browser extensions's dec_id cannot be specified as system-dec-id! req={}", 
                        req.url(), 
                    );

                    warn!("{}", msg);
                    Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
                } else {
                    // debug!("request from browser extensions: {}", req.url());
                    Ok(None)
                }
            }
            None => {
                warn!("request from unknown source, will treat as other! req={}", req.url());
                // 其余一律认为是未知的Other请求
                Ok(Some(RequestSourceString::Other))
            }
        }
    }

    fn is_browser(user_agent: &str) -> bool {
        const NONE_BROWSER_IDS: [&str; 2] = ["node-fetch", "okhttp"];
        for id in &NONE_BROWSER_IDS {
            if user_agent.find(id).is_some() {
                return false;
            }
        }

        const BROWSER_IDS: [&str; 7] = ["Mozilla", "WebKit", "Chrome", "Safari", "Gecko", "Firefox", "MSIE"];
        for id in &BROWSER_IDS {
            if user_agent.find(id).is_some() {
                return true;
            }
        }
        false
    }

    fn is_iframe(req: &http_types::Request) -> bool {
        let ret = match req.header("sec-fetch-dest") {
            Some(header) => {
                match header.last().as_str() {
                    "iframe" => {
                        true
                    }
                    _ => {
                        false
                    }
                }
            }
            None => {
                false
            }
        };

        if !ret {
            return ret;
        }

        let ret = match req.header("sec-fetch-mode") {
            Some(header) => {
                match header.last().as_str() {
                    "navigate" => {
                        true
                    }
                    _ => {
                        false
                    }
                }
            }
            None => {
                false
            }
        };

        if !ret {
            return ret;
        }

        ret
    }

    fn is_cyfs_browser_extension(req: &http_types::Request) -> bool {
        match req.header("cyfs-browser-extension") {
            Some(_header) => {
                true
            }
            None => {
                false
            }
        }
    }

    fn extract_source<'a>(req: &'a http_types::Request,) -> BuckyResult<Option<RequestSourceString<'a>>> {
        let user_agent = req.header(http_types::headers::USER_AGENT);
        debug!("req user agent: {:?}", user_agent);
        let origin = if let Some(header) = req.header(http_types::headers::ORIGIN) {
            debug!("req origin: {}", header.last().as_str());
            Some(header)
        } else if let Some(header) = req.header(http_types::headers::REFERER) {
            debug!("req referer: {}", header.last().as_str());
            Some(header)
        } else {
            None
        };

        if user_agent.is_none() && origin.is_none() {
            // request from non browser app
            return Ok(None);
        }

        // check if cyfs browser extension
        if Self::is_cyfs_browser_extension(req) {
            return Ok(Some(RequestSourceString::Extension));
        }
        
        if origin.is_none() {
            // pass through the requests from none browser env(eg. nodejs/sdk)
            let user_agent_str = user_agent.as_ref().unwrap().last().as_str();
            if !Self::is_browser(user_agent_str) {
                return Ok(None)
            }

            // check if the request open in the browser new tab address bar! 
            // Only the front protocol are allowed!
            if let Some(root) = Self::extract_front_root(req) {
                return Ok(Some(RequestSourceString::Host(root)));
            }

            // FIXME 为什么浏览器插件会发起这种不带Origin的请求
            // request from the cyfs browser extensions or none cyfs browser's html tag! now will ignore the source verify, but will disable the used of system-dec-id
            return Self::check_other_request(req);
        } else {
            // check if iframe
            if Self::is_iframe(req) {
                if let Some(root) = Self::extract_front_root(req) {
                    return Ok(Some(RequestSourceString::Host(root)));
                }
            }
        }

        let origin_url = origin.unwrap().last().as_str();
        Ok(Some(RequestSourceString::Origin(origin_url)))
    }

    fn parse_origin(req: &http_types::Request, origin_url: &str) -> BuckyResult<RequestSource> {
        match http_types::Url::parse(origin_url) {
            Ok(url) => {
                if url.scheme() == "chrome-extension" {
                    debug!("request from browser extensions: url={}, ext={}", req.url(), url.host_str().unwrap_or(""));
                    return Ok(RequestSource::Extension);
                }

                match url.host_str() {
                    Some(host) => {
                        let origin = Self::parse_host(host)?;
                        Ok(origin)
                    }
                    None => {
                        let msg = format!("parse browser request origin/referer header as url but host missing! req={}, origin={}", 
                            req.url(), origin_url);
                        warn!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
                    }
                }
            }
            Err(e) => {
                let msg = format!("parse browser request origin/referer header as url error! {}, {}", origin_url, e);
                warn!("{}", msg);
                Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg))
            }
        }
    }

    fn verify_dec(&self, mut req: http_types::Request,) -> BuckyResult<http_types::Request> {
        let ret = Self::extract_source(&req)?;
        if ret.is_none() {
            return Ok(req);
        }

        let origin = ret.unwrap();
        if self.mode == BrowserSanboxMode::Forbidden {
            let msg = format!("browser request not allowed in forbidden mode! req={}, origin={:?}", req.url(), origin);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        let allow_system_dec;
        let req_origin = match origin {
            RequestSourceString::Origin(origin) => {
                allow_system_dec = false;
                Self::parse_origin(&req, origin)?
            }
            RequestSourceString::Host(host) => {
                allow_system_dec = true;
                Self::parse_host(host)?
            }
            RequestSourceString::Extension => {
                allow_system_dec = false;
                RequestSource::Extension
            }
            RequestSourceString::Other => {
                allow_system_dec = false;
                RequestSource::Other
            }
        };
        
        if req_origin == RequestSource::System {
            return Ok(req);
        }

        match req_origin {
            RequestSource::System => {
                Ok(req)
            }
            RequestSource::Extension => {
                Self::check_extension_request(&req)?;
                Ok(req)
            }
            RequestSource::Dec(dec_id) => {
                if !allow_system_dec && dec_id == *cyfs_core::get_system_dec_app() {
                    let msg = format!("browser request dec_id not cannot be specified as system_dec_id! req={}, origin={:?}", 
                        req.url(), 
                        origin, 
                    );

                    warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
                }

                // should check header and query pairs's dec_id if matched
                let ret: Option<ObjectId> = RequestorHelper::dec_id_from_request(&req)?;
                match ret {
                    Some(source_dec_id) => {
                        if source_dec_id != dec_id {
                            let msg = format!("browser request dec_id and origin dec_id not matched! req={}, origin={:?}, source dec={}, origin dec={}", 
                                req.url(), 
                                origin, 
                                cyfs_core::dec_id_to_string(&source_dec_id), 
                                cyfs_core::dec_id_to_string(&dec_id),
                            );

                            warn!("{}", msg);
                            Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
                        } else {
                            Ok(req)
                        }
                    }
                    None => {
                        warn!("browser dec request but dec_id header or query pairs missing! req={}, origin={:?}", req.url(), origin);
                        drop(origin);

                        // insert the origin dec_id
                        req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
                        Ok(req)
                    }
                }
            }
            RequestSource::Other => {
                match self.mode {
                    BrowserSanboxMode::Strict => {
                        let msg = format!("unknown browser request not allowed in strict mode! req={}, origin={:?}", req.url(), origin);
                        warn!("{}", msg);
                        Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
                    }
                    BrowserSanboxMode::Relaxed => {
                        drop(origin);

                        // set as anonymous dec
                        req.insert_header(cyfs_base::CYFS_DEC_ID, cyfs_core::get_anonymous_dec_app().to_string());
                        Ok(req)
                    }
                    _ => unreachable!(),
                }
            }
        }

    }
}

#[async_trait::async_trait]
impl HttpServerHandler for BrowserSanboxHttpServer {
    async fn respond(
        &self,
        source: HttpRequestSource,
        mut req: http_types::Request,
    ) -> http_types::Result<http_types::Response> {
        if source.is_local() && req.method() != http_types::Method::Options {
            let ret = self.verify_dec(req);
            match ret {
                Ok(mreq) => {
                    req = mreq;
                }
                Err(e) => {
                    return Ok(RequestorHelper::trans_error(e));
                }
            }
        }
        
        self.handler.respond(source, req).await
    }
}

/*
Disable all the requests from browser
*/
pub(crate) struct DisableBrowserRequestHttpServer {
    handler: HttpServerHandlerRef,
}

impl DisableBrowserRequestHttpServer {
    pub fn new(handler: HttpServerHandlerRef) -> Self {
        Self {
            handler,
        }
    }

    pub fn into(self) -> HttpServerHandlerRef {
        Arc::new(Box::new(self))
    }

    fn check_browser_request(req: &http_types::Request) -> BuckyResult<()> {
        let user_agent = req.header(http_types::headers::USER_AGENT);
        // debug!("req user agent: {:?}", user_agent);
        let origin = if let Some(header) = req.header(http_types::headers::ORIGIN) {
            // debug!("req origin: {}", header.last().as_str());
            Some(header)
        } else if let Some(header) = req.header(http_types::headers::REFERER) {
            // debug!("req referer: {}", header.last().as_str());
            Some(header)
        } else {
            None
        };

        if user_agent.is_none() && origin.is_none() {
            // request from non browser app
            return Ok(());
        }

        if origin.is_none() {
            // pass through the requests from none browser env(eg. nodejs/sdk)
            let user_agent_str = user_agent.as_ref().unwrap().last().as_str();
            if !BrowserSanboxHttpServer::is_browser(user_agent_str) {
                return Ok(());
            }
        }

        let msg = format!("browser request not allowed on current interface! req={}, user_agent={:?}, origin={:?}", 
            req.url(), 
            user_agent, 
            origin
        );
        warn!("{}", msg);
        Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
    }
}

#[async_trait::async_trait]
impl HttpServerHandler for DisableBrowserRequestHttpServer {
    async fn respond(
        &self,
        source: HttpRequestSource,
        req: http_types::Request,
    ) -> http_types::Result<http_types::Response> {
        if source.is_local() && req.method() != http_types::Method::Options {
            Self::check_browser_request(&req)?;
        }
        
        self.handler.respond(source, req).await
    }
}

#[cfg(test)]
mod test {

    #[test]
    fn test_front_url() {
        let url = "http://127.0.0.1:21000/r.9tGpLNnSzxs7kX2pbe27adjNjGQTgFzMCR9pDQ4rHRpM/$/9tGpLNnSzxs7kX2pbe27adjNjGQTgFzMCR9pDQ4rHRpM/.cyfs/meta/root-state?format=json";

        let url = http_types::Url::parse(url).unwrap();
        let mut ret = url.path().trim_start_matches('/').split('/');
        //let first = ret.next();
        //println!("{:?}", first);

        let host = ret.next();
        if host.is_none() {
            unreachable!();
        }

        let host = host.unwrap();
        if crate::front::parse_front_host(host).is_none() {
            unreachable!();
        }

        println!("{}", host);

        let origin = "chrome-extension://ehneemhfjdafhgiddamkeglfkmcljmpe";
        let url = http_types::Url::parse(origin).unwrap();
        println!("host={:?}", url.host_str());
        let id = url.path().trim_start_matches('/');
        println!("id={}", id);
    }
}
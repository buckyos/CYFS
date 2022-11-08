use cyfs_base::*;
use super::http_server::*;
use cyfs_lib::*;

use std::sync::Arc;

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub enum BrowserSanboxMode {
    Forbidden,
    Strict,
    Relaxed,
}

impl BrowserSanboxMode {
    pub fn as_str(&self) -> &str {
        match *self {
            Self::Forbidden => "forbidden",
            Self::Strict => "strict",
            Self::Relaxed => "relaxed",
        }
    }
}

impl std::str::FromStr for BrowserSanboxMode {
    type Err = BuckyError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mode = match s {
            "forbidden" => Self::Forbidden,
            "strict" => Self::Strict,
            "relaxed" => Self::Relaxed,
            _ => {
                let msg = format!("unknown browser mode: {}", s);
                warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
            }
        };
        Ok(mode)
    }
}

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
enum RequestOrigin {
    System,
    Dec(ObjectId),
    Other,
}

pub(crate) struct BrowserSanboxHttpServer {
    mode: BrowserSanboxMode,
    handler: HttpServerHandlerRef,
}

impl BrowserSanboxHttpServer {
    pub(crate) fn new(
        handler: HttpServerHandlerRef,
        mode: BrowserSanboxMode,
    ) -> Self {
        Self {
            handler,
            mode,
        }
    }

    pub fn into(self) -> HttpServerHandlerRef {
        Arc::new(Box::new(self))
    }

    fn parse_host(host: &str) -> BuckyResult<RequestOrigin> {
        if host == "static" {
            return Ok(RequestOrigin::System);
        } 
        
        if let Some((_, dec_id)) = crate::front::parse_front_host_with_dec_id(host)? {
            return Ok(RequestOrigin::Dec(dec_id));
        } 

        warn!("unknown request origin/referer host! host={}", host);
        Ok(RequestOrigin::Other)
    }

    fn extract_origin<'a>(req: &'a http_types::Request,) -> BuckyResult<Option<&'a str>> {
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

        if origin.is_none() {
            // the request open in the browser new tab address bar! 
            let msg = format!("request from browser but origin/referer header missing! url={}", req.url());
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        let origin_url = origin.unwrap().last().as_str();
        Ok(Some(origin_url))
    }

    fn parse_origin(req: &http_types::Request, origin_url: &str) -> BuckyResult<RequestOrigin> {
        match http_types::Url::parse(origin_url) {
            Ok(url) => {
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
        let ret = Self::extract_origin(&req)?;
        if ret.is_none() {
            return Ok(req);
        }

        let origin = ret.unwrap();
        if self.mode == BrowserSanboxMode::Forbidden {
            let msg = format!("browser request not allowed in forbidden mode! req={}, origin={}", req.url(), origin);
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg));
        }

        let req_origin = Self::parse_origin(&req, origin)?;
        if req_origin == RequestOrigin::System {
            return Ok(req);
        }

        match req_origin {
            RequestOrigin::System => {
                Ok(req)
            }
            RequestOrigin::Dec(dec_id) => {
                // should check header and query pairs's dec_id if matched
                let ret: Option<ObjectId> = RequestorHelper::dec_id_from_request(&req)?;
                match ret {
                    Some(source_dec_id) => {
                        if source_dec_id != dec_id {
                            let msg = format!("browser request dec_id and origin dec_id not matched! req={}, origin={}, source dec={}, origin dec={}", 
                                req.url(), 
                                origin, 
                                source_dec_id, 
                                dec_id
                            );

                            warn!("{}", msg);
                            Err(BuckyError::new(BuckyErrorCode::PermissionDenied, msg))
                        } else {
                            Ok(req)
                        }
                    }
                    None => {
                        warn!("browser dec request but dec_id header or query pairs missing! req={}, origin={}", req.url(), origin);
                        drop(origin);

                        // insert the origin dec_id
                        req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
                        Ok(req)
                    }
                }
            }
            RequestOrigin::Other => {
                match self.mode {
                    BrowserSanboxMode::Strict => {
                        let msg = format!("unknown browser request not allowed in strict mode! req={}, origin={}", req.url(), origin);
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
        if source.is_local() {
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

        let msg = format!("browser request not allowed on current interface! req={}, origin={:?}", req.url(), origin);
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
        if source.is_local() {
            Self::check_browser_request(&req)?;
        }
        
        self.handler.respond(source, req).await
    }
}

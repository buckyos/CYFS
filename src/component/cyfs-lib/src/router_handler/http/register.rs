use super::super::*;
use crate::base::*;
use cyfs_base::*;
use cyfs_debug::Mutex;

use async_std::prelude::*;
use http_types::{Method, Request, Url};
use std::sync::Arc;
use std::time::Duration;

struct RouterHandlerRegisterHelper {
    chain: RouterHandlerChain,
    category: RouterHandlerCategory,
    id: String,
    dec_id: Option<ObjectId>,
    service_url: Url,
}

impl RouterHandlerRegisterHelper {
    pub fn new(
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: impl Into<String>,
        dec_id: Option<ObjectId>,
        service_url: &str,
    ) -> Self {
        let service_url = Url::parse(service_url).unwrap();
        let service_url = service_url.join("handler/non/").unwrap();
        info!("router handler service url: {}", service_url);

        Self {
            chain,
            category,
            id: id.into(),
            dec_id,
            service_url,
        }
    }

    // [base]/[handler_category]/[handler_id]
    fn gen_handler_url(&self) -> Url {
        let path = format!(
            "{}/{}/{}",
            self.chain.to_string(),
            self.category.to_string(),
            self.id
        );
        self.service_url.join(&path).unwrap()
    }

    fn gen_http_request<T>(&self, method: Method, url: Url, req: Option<&T>) -> Request
    where
        T: JsonCodec<T>,
    {
        let mut http_req = Request::new(method, url);

        if let Some(dec_id) = &self.dec_id {
            http_req.insert_header(cyfs_base::CYFS_DEC_ID, dec_id.to_string());
        }

        if req.is_some() {
            let req = req.as_ref().unwrap().encode_string();

            http_req.set_body(req);
        }

        http_req
    }

    async fn post(&self, req: Request) -> BuckyResult<()> {
        let host = self.service_url.host_str().unwrap();
        let port = self.service_url.port().unwrap();
        let addr = format!("{}:{}", host, port);
        let requestor = TcpHttpRequestor::new(&addr);

        let mut resp = requestor.request(req).await?;

        if resp.status().is_success() {
            Ok(())
        } else {
            let err = RequestorHelper::trans_status_code(resp.status());

            let resp_string = resp.body_string().await.map_err(|e| {
                let msg = format!(
                    "router handler modify resp body error! err={}, addr={}",
                    e, addr
                );
                error!("{}", msg);
                BuckyError::from(msg)
            })?;

            error!(
                "router handler modify resp error status: {}, {}",
                resp.status(),
                addr
            );

            Err(BuckyError::new(err, resp_string))
        }
    }
}

struct RouterHandlerRegisterInner {
    index: i32,
    filter: Option<String>,
    req_path: Option<String>,
    default_action: RouterHandlerAction,
    routine: Option<String>,

    // 状态, true表示运行注册，false表示停止
    status: bool,
}

impl RouterHandlerRegisterInner {
    pub fn new(
        index: i32,
        filter: Option<String>,
        req_path: Option<String>,
        default_action: RouterHandlerAction,
        routine: Option<String>,
    ) -> Self {
        Self {
            index,
            filter,
            req_path,
            default_action,
            routine,

            status: true,
        }
    }

    fn gen_add_handler_param(&self) -> RouterAddHandlerParam {
        RouterAddHandlerParam {
            filter: self.filter.clone(),
            req_path: self.req_path.clone(),
            index: self.index,
            default_action: self.default_action.clone(),
            routine: self.routine.clone(),
        }
    }
}

#[derive(Clone)]
pub(super) struct RouterHandlerRegister {
    inner: Arc<Mutex<RouterHandlerRegisterInner>>,
    helper: Arc<RouterHandlerRegisterHelper>,
}

impl RouterHandlerRegister {
    pub fn new(
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: &str,
        dec_id: Option<ObjectId>,
        index: i32,
        filter: Option<String>,
        req_path: Option<String>,
        default_action: RouterHandlerAction,
        routine: Option<String>,
        service_url: &str,
    ) -> Self {
        let inner =
            RouterHandlerRegisterInner::new(index, filter, req_path, default_action, routine);
        let helper = RouterHandlerRegisterHelper::new(chain, category, id, dec_id, service_url);

        Self {
            inner: Arc::new(Mutex::new(inner)),
            helper: Arc::new(helper),
        }
    }

    pub fn register(&self) {
        let register = self.clone();

        // 一次注册失败，并不会返回错误？
        async_std::task::spawn(async move {
            register.run_register().await;
        });
    }

    pub async fn unregister(&self) -> BuckyResult<bool> {
        // 修改状态
        {
            let mut inner = self.inner.lock().unwrap();
            if !inner.status {
                warn!(
                    "router handler register already stopped! chain={}, category={}, id={}",
                    self.helper.chain, self.helper.category, self.helper.id
                );
            }
            inner.status = false;
        }

        let helper = self.helper.clone();
        let unregister = RouterHandlerUnregister::new_from_helper(helper);
        unregister.unregister().await
    }

    pub async fn run_register(self) {
        let _r = self.register_once().await;

        // TODO 这里是否要循环注册？non-stack进程重启后如何处理？
        let mut interval = async_std::stream::interval(Duration::from_secs(60 * 10));
        while let Some(_) = interval.next().await {
            // 检查状态
            {
                let inner = self.inner.lock().unwrap();
                if !inner.status {
                    info!(
                        "router handler register stopped! chain={}, category={}, id={}",
                        self.helper.chain, self.helper.category, self.helper.id
                    );
                    break;
                }
            }

            let _r = self.register_once().await;
        }
    }

    async fn register_once(&self) -> BuckyResult<()> {
        let req = {
            let url = self.helper.gen_handler_url();

            let inner = self.inner.lock().unwrap();
            let req = inner.gen_add_handler_param();
            self.helper.gen_http_request(Method::Post, url, Some(&req))
        };

        let url = req.url().clone();
        match self.helper.post(req).await {
            Ok(_) => {
                info!("add router handler success! url={}", url);
                Ok(())
            }
            Err(e) => {
                error!("add router handler error! url={}, err={}", url, e);
                Err(e)
            }
        }
    }
}

pub(super) struct RouterHandlerUnregister {
    helper: Arc<RouterHandlerRegisterHelper>,
}

impl RouterHandlerUnregister {
    pub fn new(
        chain: RouterHandlerChain,
        category: RouterHandlerCategory,
        id: impl Into<String>,
        dec_id: Option<ObjectId>,
        service_url: &str,
    ) -> Self {
        let helper = RouterHandlerRegisterHelper::new(chain, category, id, dec_id, service_url);

        Self {
            helper: Arc::new(helper),
        }
    }

    fn new_from_helper(helper: Arc<RouterHandlerRegisterHelper>) -> Self {
        Self { helper }
    }

    fn gen_remove_handler_param(&self) -> RouterRemoveHandlerParam {
        RouterRemoveHandlerParam {
            id: self.helper.id.clone(),
        }
    }

    pub async fn unregister(&self) -> BuckyResult<bool> {
        let req = {
            let url = self.helper.gen_handler_url();
            self.helper
                .gen_http_request::<RouterRemoveHandlerParam>(Method::Delete, url, None)
        };

        let url = req.url().clone();
        match self.helper.post(req).await {
            Ok(_) => {
                info!("remove router handler success! url={}", url);
                Ok(true)
            }
            Err(e) => match e.code() {
                BuckyErrorCode::NotFound => {
                    warn!("remove router handler but not found! url={} err={}", url, e);
                    Ok(false)
                }
                _ => {
                    error!("remove router handler error! url={} err={}", url, e);
                    Err(e)
                }
            },
        }
    }
}

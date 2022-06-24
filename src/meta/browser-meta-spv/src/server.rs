use std::sync::Arc;
use crate::storage::Storage;
use crate::status::Status;
use crate::Config;
use tide::security::CorsMiddleware;
use tide::http::headers::HeaderValue;
use tide::{Request, Response};
use serde::Deserialize;
use cyfs_base::bucky_time_now;

pub struct SPVServer {
    storage: Arc<Box<dyn Storage + Send + Sync>>,
    status: Arc<Status>,
    app: tide::Server<()>,
    endpoint: String
}

#[derive(Deserialize)]
#[serde(default)]
struct GetBlockParam {
    begin: i64,
    end: i64,
    caller: Option<String>,
    to: Option<String>,
    pages: usize,
    limit: usize
}

impl Default for GetBlockParam {
    fn default() -> Self {
        Self {
            begin: 0,
            end: -1,
            caller: None,
            to: None,
            pages: 0,
            limit: 20
        }
    }
}

impl SPVServer {
    pub(crate) fn new(config: &Config, storage: Arc<Box<dyn Storage + Send + Sync>>, status: Arc<Status>) -> Self {
        let mut app = tide::new();
        let cors = CorsMiddleware::new()
            .allow_methods(
                "GET, POST, PUT, DELETE, OPTIONS"
                    .parse::<HeaderValue>()
                    .unwrap(),
            )
            .allow_origin("*")
            .allow_credentials(true)
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .expose_headers("*".parse::<HeaderValue>().unwrap());
        app.with(cors);

        Self {
            storage,
            status,
            app,
            endpoint: config.service_endpoint.clone()
        }
    }

    pub fn register(&mut self) {
        let status1 = self.status.clone();
        self.app.at("/status").get(move |_req: Request<()>| {
            let status = status1.clone();
            async move {
                let height = status.cur_height();
                let tx_num = status.tx_num();
                let mut resp = Response::new(tide::http::StatusCode::Ok);
                resp.set_body(serde_json::json!({"blocks": height, "transactions": tx_num, "cur_time": bucky_time_now()}));
                Ok(resp)
            }

        });

        let status2 = self.status.clone();
        let storage1 = self.storage.clone();
        self.app.at("/blocks").get(move |req: Request<()>| {
            let storage = storage1.clone();
            let status = status2.clone();
            async move {
                let mut resp = Response::new(tide::http::StatusCode::Ok);
                let mut param: GetBlockParam = req.query()?;
                if param.end == -1 {
                    param.end = status.cur_height();
                }
                let ret = storage.get_blocks(param.begin, param.end, param.pages, param.limit).await?;
                resp.set_body(serde_json::json!(ret));
                Ok(resp)
            }
        });

        let status3 = self.status.clone();
        let storage2 = self.storage.clone();
        self.app.at("/txs").get(move |req: Request<()>| {
            let storage = storage2.clone();
            let status = status3.clone();
            async move {
                let mut resp = Response::new(tide::http::StatusCode::Ok);
                let mut param: GetBlockParam = req.query()?;
                if param.end == -1 {
                    param.end = status.cur_height();
                }
                let ret = storage.get_txs(param.begin, param.end, param.caller, param.to, param.pages, param.limit).await?;
                resp.set_body(serde_json::json!(ret));
                Ok(resp)
            }
        });

        let storage3 = self.storage.clone();
        self.app.at("/tx/:id").get(move |req: Request<()>| {
            let storage = storage3.clone();
            async move {
                if let Ok(tx_id) = req.param("id") {
                    let mut resp = Response::new(tide::http::StatusCode::Ok);
                    let (info, tx_raw, receipt_raw) = storage.get_tx(&tx_id).await?;
                    resp.set_body(serde_json::json!({"info": info, "tx_raw": hex::encode(tx_raw), "tx_receipt": hex::encode(receipt_raw)}));
                    Ok(resp)
                } else {
                    Ok(Response::new(tide::http::StatusCode::NotFound))
                }

            }
        });

        let storage4 = self.storage.clone();
        self.app.at("/account/:id").get(move |req: Request<()>| {
            let storage = storage4.clone();
            async move {
                if let Ok(tx_id) = req.param("id") {
                    let mut resp = Response::new(tide::http::StatusCode::Ok);
                    let info = storage.get_account_info(&tx_id).await?;
                    resp.set_body(serde_json::json!(info));
                    Ok(resp)
                } else {
                    Ok(Response::new(tide::http::StatusCode::NotFound))
                }

            }
        });
    }

    pub async fn run(self) {
        log::info!("start http server:{}", &self.endpoint);
        self.app.listen(self.endpoint.as_str()).await.unwrap();
    }
}
use crate::*;
use cyfs_base::*;
use std::sync::Arc;
use tide::Server;
use std::convert::TryFrom;
use tide::{Request, Response, StatusCode};
use std::str::FromStr;
use tide::http::Url;
use log::*;
use http_types::headers::HeaderValue;
use tide::security::{CorsMiddleware, Origin};

pub struct MetaHttpServer {
    pub miner: Arc<dyn Miner>,
    pub app: Server<()>,
    server_port: u16,
}

fn object_id_from_req_params(url: &Url) -> Result<ObjectId, std::io::Error> {
    match url.query_pairs().find(|(x, _)| x == "id") {
        Some((_, id_str)) => {
            return ObjectId::from_str(&id_str).map_err(|_err| std::io::Error::from(std::io::ErrorKind::NotFound));
        },
        _ => {
            return Err(std::io::Error::from(std::io::ErrorKind::NotFound));
        }
    };
}

fn txhash_from_req_params(url: &Url) -> Result<TxHash, std::io::Error> {
    return match url.query_pairs().find(|(x, _)| x == "tx") {
        Some((_, id_str)) => {
            let hash = TxHash::from_str(id_str.as_ref()).map_err(|_| std::io::Error::from(std::io::ErrorKind::NotFound))?;
            Ok(hash)
        },
        _ => {
            Err(std::io::Error::from(std::io::ErrorKind::NotFound))
        }
    };
}

impl MetaHttpServer {
    pub fn new(miner: Arc<dyn Miner>, server_port: u16) -> Self {
        let mut app = tide::new();

        let cors = CorsMiddleware::new()
            .allow_methods(
                "GET, POST, PUT, DELETE, OPTIONS"
                    .parse::<HeaderValue>()
                    .unwrap(),
            )
            .allow_origin(Origin::from("*"))
            .allow_credentials(true)
            .allow_headers("*".parse::<HeaderValue>().unwrap())
            .expose_headers("*".parse::<HeaderValue>().unwrap());
        app.with(cors);

        let tmp_miner = miner.clone();
        app.at("/nonce").get(move |req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let account = object_id_from_req_params(&req.url())?;
                let result = miner.get_nonce(&account).await.or_else(|_| Err(ERROR_EXCEPTION));

                let body_str = result.to_hex()?;
                debug!("nonce request:{} {}", account.to_string(), body_str);
                let mut resp = Response::new(StatusCode::Ok);
                resp.set_body(body_str);
                Ok(resp)
            }
        });
        let tmp_miner = miner.clone();
        app.at("/receipt").get(move |_req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let txid = txhash_from_req_params(&_req.url())?;
                let mut resp = Response::new(StatusCode::Ok);
                let result = miner.as_chain().get_chain_storage().receipt_of(&txid).await.or_else(|_| Err(ERROR_EXCEPTION));

                resp.set_body(result.to_hex()?);
                Ok(resp)
            }
        });
        let tmp_miner = miner.clone();
        app.at("/commit").post(move |mut req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let tx_body = req.body_bytes().await?;
                // log::debug!("commit tx {:?}", tx_str);
                let tx = MetaTx::clone_from_slice(tx_body.as_slice())?;
                info!("commit tx {} caller {} nonce {} max_fee {} gas_price {}", tx.desc().calculate_id().to_string(),
                       tx.desc().content().caller.id()?.to_string(),
                       tx.desc().content().nonce, tx.desc().content().max_fee, tx.desc().content().gas_price);
                let result;
                if tx.desc().content().max_fee < 10 || tx.desc().content().gas_price < 10 {
                    result = Err(ERROR_NOT_ENOUGH_FEE);
                } else {
                    let public_key_ret = {
                        let storage = miner.as_chain().get_chain_storage().state_storage();
                        let ref_state = storage.create_state(true).await;
                        let account_info_ret = ref_state.get_account_info(&tx.desc().content().caller.id()?).await;
                        if let Err(err) = &account_info_ret {
                            if let ERROR_NOT_FOUND = get_meta_err_code(&err)? {
                                Ok(tx.desc().content().caller.get_public_key()?.clone())
                            } else {
                                Err(account_info_ret.err().unwrap())
                            }
                        } else {
                            Ok(account_info_ret.unwrap().get_public_key()?.clone())
                        }
                    };

                    if public_key_ret.is_ok() {
                        let public_key = public_key_ret.unwrap();
                        if !tx.async_verify_signature(public_key).await? {
                            result = Err(ERROR_SIGNATURE_ERROR);
                        } else {
                            let tx_hash = tx.desc().calculate_id();
                            match miner.push_tx(tx).await {
                                Result::Err(e) => {
                                    result = Err(ERROR_BUCKY_ERR_START + e.code().into_u16());
                                },
                                Result::Ok(_) => {
                                    result = Ok(TxId::try_from(tx_hash)?);
                                }
                            }
                        }
                    } else {
                        result = Err(ERROR_PUBLIC_KEY_NOT_EXIST);
                    }
                }
                // API 调用记录日志
                if let Some(stat) = miner.as_chain().get_stat() {
                    stat.api_call("commit", *result.as_ref().err().unwrap_or(&0))
                }

                let body_str = result.to_hex()?;
                let mut resp = Response::new(StatusCode::Ok);
                resp.set_body(body_str);
                Ok(resp)
            }
        });
        let tmp_miner = miner.clone();
        app.at("/view").post(move |mut req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let request: ViewRequest = ViewRequest::clone_from_hex(req.body_string().await?.as_str(), &mut Vec::new())?;
                let name = request.method.method_name();
                let stat = miner.as_chain().get_stat();
                let result = miner.as_chain().get_chain_storage().view(request, stat.clone()).await.or_else(|e| {
                    if let BuckyErrorCode::MetaError(code) = e.code() {
                        Err(code)
                    } else {
                        Err(ERROR_EXCEPTION)
                    }
                });
                // API 调用记录日志
                if let Some(stat) = stat {
                    stat.api_call(&format!("view:{}", name), *result.as_ref().err().unwrap_or(&0))
                }

                let body_str = result.to_hex().unwrap();
                let mut resp = Response::new(tide::http::StatusCode::Ok);
                resp.set_body(body_str);
                Ok(resp)
            }
        });

        let tmp_miner = miner.clone();
        app.at("/status").get(move |_req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let result = match miner.as_chain().get_chain_storage().get_status().await {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    }
                    Err(e) => {
                        info!("get balance error.{}", e);
                        RequestResult::from_err(e)
                    }
                };

                let body_str = serde_json::to_string(&result).unwrap();
                let mut resp = Response::new(tide::http::StatusCode::Ok);
                resp.set_content_type("application/json");
                resp.set_body(body_str);
                Ok(resp)
            }
        });

        let tmp_miner = miner.clone();
        app.at("/balance").post(move |mut req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let address_list: Vec<(u8, String)> = req.body_json().await?;
                let result = match miner.as_chain().get_chain_storage().get_balance(address_list).await {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    }
                    Err(e) => {
                        info!("get balance error.{}", e);
                        RequestResult::from_err(e)
                    }
                };
                // API 调用记录日志
                if let Some(stat) = miner.as_chain().get_stat() {
                    stat.api_call("balance", result.err)
                }

                let body_str = serde_json::to_string(&result).unwrap();
                let mut resp = Response::new(tide::http::StatusCode::Ok);
                resp.set_content_type("application/json");
                resp.set_body(body_str);
                Ok(resp)
            }
        });

        let tmp_miner = miner.clone();
        app.at("/tx/:tx_hash").get(move |req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let tx_hash: String = req.param("tx_hash")?.to_string();
                let result = match {
                    miner.as_chain().get_chain_storage().get_tx_info(&TxHash::from_str(tx_hash.as_str())?).await
                } {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    }
                    Err(e) => {
                        info!("get tx {} error.{}", tx_hash, e);
                        RequestResult::from_err(e)
                    }
                };

                let body_str = serde_json::to_string(&result).unwrap();
                let mut resp = Response::new(tide::http::StatusCode::Ok);
                resp.set_content_type("application/json");
                resp.set_body(body_str);
                Ok(resp)
            }
        });

        let tmp_miner = miner.clone();
        app.at("/tx_full/:tx_hash").get(move |req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let tx_hash: String = req.param("tx_hash")?.to_string();
                let result = match {
                    miner.as_chain().get_chain_storage().get_tx_full_info(&TxHash::from_str(tx_hash.as_str())?).await
                } {
                    Ok(ret) => {
                        Ok(ret)
                    }
                    Err(e) => {
                        info!("get tx {} error.{}", tx_hash, e);
                        Err(ERROR_EXCEPTION)
                    }
                };

                let mut resp = Response::new(tide::http::StatusCode::Ok);
                resp.set_body(result.to_vec().unwrap());
                Ok(resp)
            }
        });

        let tmp_miner = miner.clone();
        app.at("/blocks").post(move |mut req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let req_param: GetBlocksRequest = req.body_json().await?;
                let result = match miner.as_chain().get_chain_storage().get_blocks_info_by_range(req_param.start_block, req_param.end_block).await {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    }
                    Err(e) => {
                        info!("get balance error.{}", e);
                        RequestResult::from_err(e)
                    }
                };

                let body_str = serde_json::to_string(&result).unwrap();
                let mut resp = Response::new(tide::http::StatusCode::Ok);
                resp.set_content_type("application/json");
                resp.set_body(body_str);
                Ok(resp)
            }
        });

        let tmp_miner = miner.clone();
        app.at("/stat").get(move |_req: Request<()>| {
            let miner = tmp_miner.clone();
            async move {
                let resp = if let Some(stat) = miner.as_chain().get_stat() {
                    let stat = stat.get_memory_stat();
                    let mut resp = Response::new(StatusCode::Ok);
                    resp.set_content_type("application/json");
                    resp.set_body(serde_json::to_value(stat)?);
                    resp
                } else {
                    Response::new(StatusCode::NotFound)
                };
                Ok(resp)
            }
        });

        Self {
            miner,
            app,
            server_port,
        }
    }

    pub async fn run(self) -> BuckyResult<()> {
        let addr = format!("0.0.0.0:{}", self.server_port);
        log::info!("start http server:{}", addr);
        self.app.listen(addr).await?;
        Ok(())
    }
}

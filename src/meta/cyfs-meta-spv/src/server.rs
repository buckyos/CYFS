use crate::*;
use cyfs_base::*;
use tide::Server;
use tide::{Request, Response};
use std::str::FromStr;
use tide::http::Url;
use log::*;
use cyfs_base_meta::*;
use serde_json::{Value};
use tide::security::{CorsMiddleware, Origin};
use tide::http::headers::HeaderValue;

pub struct SPVHttpServer {
    pub spv_storage: SPVChainStorageRef,
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

impl SPVHttpServer {
    pub fn new(storage: SPVChainStorageRef, server_port: u16) -> Self {
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

        let tmp_storage = storage.clone();
        app.at("/collect_tx_list").post(move |mut req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let req_param: GetTxListRequest = req.body_json().await?;
                let tx_storage = storage.create_tx_storage().await?;
                let result = match tx_storage.get_collect_tx_list(
                    req_param.address_list.clone(),
                    req_param.block_section.clone(),
                    req_param.offset,
                    req_param.length,
                    req_param.coin_id_list.clone()).await {
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

        let tmp_storage = storage.clone();
        app.at("/payment_tx_list").post(move |mut req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let req_param: GetTxListRequest = req.body_json().await?;
                let tx_storage = storage.create_tx_storage().await?;
                let result = match tx_storage.get_payment_tx_list(
                    req_param.address_list.clone(),
                    req_param.block_section.clone(),
                    req_param.offset,
                    req_param.length,
                    req_param.coin_id_list.clone()).await {
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

        let tmp_storage = storage.clone();
        app.at("/tx_list").post(move |mut req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let req_param: GetTxListRequest = req.body_json().await?;
                let tx_storage = storage.create_tx_storage().await?;
                let result = match tx_storage.get_tx_list(req_param.address_list, req_param.block_section, req_param.offset, req_param.length, req_param.coin_id_list).await {
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

        let tmp_storage = storage.clone();
        app.at("/reward_amount/:address").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let address: &str = req.param("address")?;
                let tx_storage = storage.create_tx_storage().await?;
                let result = match {
                    tx_storage.get_file_amount(address.to_string()).await
                } {
                    Ok(ret) => {
                        RequestResult::from(ret.to_string())
                    }
                    Err(e) => {
                        info!("get reward amount error.{}", e);
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

        let tmp_storage = storage.clone();
        app.at("/query_auth_contract/:service_id/:user_id").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let service_id: &str = req.param("service_id")?;
                let user_id: &str = req.param("user_id")?;
                let tx_storage = storage.create_tx_storage().await?;
                let result = tx_storage.get_auth_contract(service_id,
                                                                                  user_id).await.or_else(|e| {
                    if let BuckyErrorCode::MetaError(code) = e.code() {
                        Err(code)
                    } else {
                        Err(ERROR_EXCEPTION)
                    }
                });

                let mut resp = Response::new(tide::http::StatusCode::Ok);
                resp.set_body(result.to_vec()?);
                Ok(resp)
            }
        });

        let tmp_storage = storage.clone();
        app.at("/status").get(move |_req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {

                let tx_storage = storage.create_tx_storage().await?;
                let result = match tx_storage.get_status().await {
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

        let tmp_storage = storage.clone();
        app.at("/erc20_contract_tx").post(move |mut req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let req_param: Value  = req.body_json().await?;
                let address = req_param["address"].as_str().unwrap_or("");
                let tx_hash = req_param["tx_hash"].as_str().unwrap_or("");
                let from = req_param["from"].as_str().unwrap_or("");
                let to = req_param["to"].as_str().unwrap_or("");
                let start_number = req_param["start_number"].as_i64().unwrap_or(0);
                let end_number = req_param["end_number"].as_i64().unwrap_or(i64::MAX);

                //info!("{}, {}, {}, {}, {}, {}", address, tx_hash, from, to, start_number, end_number);
                //let req_param: ERC20ContractTx = req.body_json().await?;
                let tx_storage = storage.create_tx_storage().await?;
                let result = match {
                    tx_storage.get_erc20_contract_tx(
                        address,
                        tx_hash,
                        start_number,
                        end_number,
                        from,
                        to).await
                } {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    }
                    Err(e) => {
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

        let tmp_storage = storage.clone();
        app.at("/nft_get/:nft_id").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let ret: BuckyResult<NFTData> = async move {
                    let nft_id_str = req.param("nft_id")?;
                    let nft_id = ObjectId::from_str(nft_id_str)?;

                    let tx_storage = storage.create_tx_storage().await?;

                    let file_amount = tx_storage.get_file_amount(nft_id_str.to_string()).await?;

                    let nft_data = match tx_storage.nft_get(&nft_id).await {
                        Ok(nft_detail) => {
                            NFTData {
                                nft_id: nft_id_str.to_string(),
                                create_time: bucky_time_to_js_time(nft_detail.desc.nft_create_time()),
                                beneficiary: nft_detail.beneficiary.to_string(),
                                owner_id: nft_detail.desc.owner_id().as_ref().unwrap().to_string(),
                                author_id: nft_detail.desc.author_id().as_ref().unwrap().to_string(),
                                name: nft_detail.name.clone(),
                                reward_amount: file_amount,
                                like_count: nft_detail.like_count,
                                state: nft_detail.state,
                                block_number: nft_detail.block_number,
                                parent_id: nft_detail.desc.parent_id().map(|item| item.to_string()),
                                sub_list: nft_detail.desc.sub_list().map(|item| item.iter().map(|sub| sub.to_string()).collect()),
                                price: nft_detail.price,
                                coin_id: nft_detail.coin_id
                            }
                        },
                        Err(e) => {
                            if get_meta_err_code(&e)? == ERROR_NOT_FOUND {
                                let like_count = tx_storage.nft_get_likes_count(&nft_id).await?;
                                NFTData {
                                    nft_id: nft_id_str.to_string(),
                                    create_time: 0,
                                    beneficiary: "".to_string(),
                                    owner_id: "".to_string(),
                                    author_id: "".to_string(),
                                    name: "".to_string(),
                                    reward_amount: file_amount,
                                    like_count: like_count as i64,
                                    state: NFTState::Normal,
                                    block_number: 0,
                                    parent_id: None,
                                    sub_list: None,
                                    price: 0,
                                    coin_id: CoinTokenId::Coin(0)
                                }
                            } else {
                                return Err(e);
                            }
                        }
                    };

                    Ok(nft_data)
                }.await;

                let result = match ret {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    },
                    Err(e) => {
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

        let tmp_storage = storage.clone();
        app.at("/nft_get_price/:nft_id").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let ret: BuckyResult<(u64, CoinTokenId)> = async move {
                    let nft_id_str = req.param("nft_id")?;

                    let tx_storage = storage.create_tx_storage().await?;

                    let file_amount = match tx_storage.nft_get_price(nft_id_str).await {
                        Ok(ret) => ret,
                        Err(_) => (0, CoinTokenId::Coin(0))
                    };

                    Ok(file_amount)
                }.await;

                let result = match ret {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    },
                    Err(e) => {
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

        let tmp_storage = storage.clone();
        app.at("/nft_get_changed_price_of_creator/:creator/:latest_height").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let ret: BuckyResult<Vec<(String, u64, CoinTokenId, u64)>> = async move {
                    let creator_str = req.param("creator")?;
                    let latest_height: u64 = req.param("latest_height")?.parse()?;

                    let tx_storage = storage.create_tx_storage().await?;

                    let file_amount = tx_storage.nft_get_changed_price_of_creator(creator_str, latest_height).await?;

                    Ok(file_amount)
                }.await;

                let result = match ret {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    },
                    Err(e) => {
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

        let tmp_storage = storage.clone();
        app.at("/nft_get_bid_list/:nft_id").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let ret: BuckyResult<Vec<NFTBidRecord>> = async move {
                    let nft_id_str = req.param("nft_id")?;
                    let nft_id = ObjectId::from_str(nft_id_str)?;

                    let tx_storage = storage.create_tx_storage().await?;
                    let list = tx_storage.nft_get_bid_list(&nft_id, 0, 100).await?;
                    let mut ret_list = Vec::new();
                    for (buyer_id, price, coin_id) in list.into_iter() {
                        ret_list.push(NFTBidRecord {
                            buyer_id: buyer_id.to_string(),
                            price,
                            coin_id
                        });
                    }

                    Ok(ret_list)
                }.await;

                let result = match ret {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    },
                    Err(e) => {
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

        let tmp_storage = storage.clone();
        app.at("/nft_get_latest_likes/:nft_id/:count").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let ret: BuckyResult<Vec<(ObjectId, u64, String)>> = async move {
                    let nft_id_str = req.param("nft_id")?;
                    let nft_id = ObjectId::from_str(nft_id_str)?;
                    let count: u64 = req.param("count")?.parse()?;

                    let tx_storage = storage.create_tx_storage().await?;
                    let list = tx_storage.nft_get_latest_likes(&nft_id, count).await?.into_iter().map(|item| {
                        (item.0, item.1, item.2.to_string())
                    }).collect();
                    Ok(list)
                }.await;

                let result = match ret {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    },
                    Err(e) => {
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

        let tmp_storage = storage.clone();
        app.at("/nft_get_of_user/:user_id/:latest_height").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let ret: BuckyResult<Vec<NFTData>> = async move {
                    let nft_id_str = req.param("user_id")?;
                    let latest_height: i64 = req.param("latest_height")?.parse()?;
                    let nft_id = ObjectId::from_str(nft_id_str)?;

                    let tx_storage = storage.create_tx_storage().await?;
                    let list = tx_storage.nft_get_latest_of_user(&nft_id, latest_height).await?;
                    let mut data_list = Vec::new();
                    for nft_detail in list.iter() {
                        data_list.push(
                            NFTData {
                                nft_id: nft_detail.desc.nft_id().to_string(),
                                create_time: bucky_time_to_js_time(nft_detail.desc.nft_create_time()),
                                beneficiary: nft_detail.beneficiary.to_string(),
                                owner_id: nft_detail.desc.owner_id().as_ref().unwrap().to_string(),
                                author_id: nft_detail.desc.author_id().as_ref().unwrap().to_string(),
                                name: nft_detail.name.clone(),
                                reward_amount: 0,
                                like_count: nft_detail.like_count,
                                state: nft_detail.state.clone(),
                                block_number: nft_detail.block_number,
                                parent_id: nft_detail.desc.parent_id().map(|i| i.to_string()),
                                sub_list: nft_detail.desc.sub_list().map(|i| i.iter().map(|x| x.to_string()).collect()),
                                price: nft_detail.price,
                                coin_id: nft_detail.coin_id.clone()
                            });
                    }
                    Ok(data_list)
                }.await;

                let result = match ret {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    },
                    Err(e) => {
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

        let tmp_storage = storage.clone();
        app.at("/nft_get_latest_transfer/:user_id/:latest_height").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let ret: BuckyResult<Vec<NFTTransferRecord>> = async move {
                    let nft_id_str = req.param("user_id")?;
                    let latest_height: i64 = req.param("latest_height")?.parse()?;

                    let tx_storage = storage.create_tx_storage().await?;
                    let list = tx_storage.nft_get_latest_transfer(nft_id_str, latest_height).await?;
                    let mut data_list = Vec::new();
                    for nft_detail in list.iter() {
                        data_list.push(
                            NFTTransferRecord {
                                nft_id: nft_detail.desc.nft_id().to_string(),
                                create_time: nft_detail.desc.nft_create_time(),
                                owner_id: nft_detail.desc.owner_id().as_ref().unwrap().to_string(),
                                author_id: nft_detail.desc.author_id().as_ref().unwrap().to_string(),
                                name: nft_detail.name.clone(),
                                block_number: nft_detail.block_number,
                                from: nft_detail.from.clone(),
                                to: nft_detail.to.clone(),
                                cached: nft_detail.nft_cached.clone()
                            });
                    }
                    Ok(data_list)
                }.await;

                let result = match ret {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    },
                    Err(e) => {
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

        let tmp_storage = storage.clone();
        app.at("/nft_get_creator_latest_transfer/:user_id/:latest_height").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let ret: BuckyResult<Vec<NFTTransferRecord>> = async move {
                    let nft_id_str = req.param("user_id")?;
                    let latest_height: i64 = req.param("latest_height")?.parse()?;

                    let tx_storage = storage.create_tx_storage().await?;
                    let list = tx_storage.nft_get_creator_latest_transfer(nft_id_str, latest_height).await?;
                    let mut data_list = Vec::new();
                    for nft_detail in list.iter() {
                        data_list.push(
                            NFTTransferRecord {
                                nft_id: nft_detail.desc.nft_id().to_string(),
                                create_time: nft_detail.desc.nft_create_time(),
                                owner_id: nft_detail.desc.owner_id().as_ref().unwrap().to_string(),
                                author_id: nft_detail.desc.author_id().as_ref().unwrap().to_string(),
                                name: nft_detail.name.clone(),
                                block_number: nft_detail.block_number,
                                from: nft_detail.from.clone(),
                                to: nft_detail.to.clone(),
                                cached: nft_detail.nft_cached.clone()
                            });
                    }
                    Ok(data_list)
                }.await;

                let result = match ret {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    },
                    Err(e) => {
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

        let tmp_storage = storage.clone();
        app.at("/nft_has_like/:nft_id/:user_id").get(move |req: Request<()>| {
            let storage = tmp_storage.clone();
            async move {
                let ret: BuckyResult<bool> = async move {
                    let nft_id_str = req.param("nft_id")?;
                    let nft_id = ObjectId::from_str(nft_id_str)?;
                    let user_id_str = req.param("user_id")?;
                    let user_id = ObjectId::from_str(user_id_str)?;

                    let tx_storage = storage.create_tx_storage().await?;
                    let has = tx_storage.nft_has_like(&nft_id, &user_id).await?;
                    Ok(has)
                }.await;

                let result = match ret {
                    Ok(ret) => {
                        RequestResult::from(ret)
                    },
                    Err(e) => {
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

        Self {
            spv_storage: storage,
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

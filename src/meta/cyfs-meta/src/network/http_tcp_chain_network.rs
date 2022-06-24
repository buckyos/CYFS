use crate::network::{ChainNetwork, ChainNetworkEventEndpoint};
use cyfs_base::{BuckyResult, BuckyError};
use async_std::net::{TcpListener, TcpStream};
use http_types::{Response, Request, Method, StatusCode, Url};
use async_trait::async_trait;
use log::*;
use futures::StreamExt;
use async_std::sync::Arc;
use std::sync::Mutex;
use serde::{Serialize, Deserialize};
use rand::Rng;
use http_types::headers::CONTENT_LENGTH;
use std::time::Duration;
use crate::*;

#[derive(Serialize, Deserialize)]
pub struct HttpAddress {
    pub port: u16,
    pub address_list: Vec<String>,
}

pub struct HttpTcpChainNetwork {
    port: u16,
    node_list: Mutex<Vec<(String, String)>>,
    is_stop: Arc<Mutex<bool>>,
}

impl HttpTcpChainNetwork {
    pub fn new(port: u16, node_list: Vec<(String, String)>) -> Self {
        Self {
            port,
            node_list: Mutex::new(node_list),
            is_stop: Arc::new(Mutex::new(false))
        }
    }

    async fn broadcast_inner(obj: &Vec<u8>, node_list: &Vec<String>) -> Vec<BuckyResult<Response>> {
        let mut tasks = Vec::new();
        for node in node_list {
            let node = node.clone();
            let mut req = Request::new(Method::Post, Url::parse(format!("http://{}", node).as_str()).unwrap());
            req.insert_header(CONTENT_LENGTH, obj.len().to_string());
            req.set_body(obj.clone());

            let task = async_std::task::spawn(async move {
                match async_std::future::timeout(Duration::new(20, 0), async move {
                    let stream = TcpStream::connect(node.as_str()).await.map_err(|err| {
                        error!("connect to node failed! node={}, err={}", node.as_str(), err);
                        err
                    })?;

                    let resp = async_h1::connect(stream, req).await.map_err(|err| {
                        error!("http connect error! node={}, err={}", node.as_str(), err);
                        err
                    })?;
                    info!("connect {} complete", node.as_str());

                    BuckyResult::Ok(resp)
                }).await {
                    Ok(ret) => {
                        ret
                    }
                    Err(err) => {
                        info!("connect timeout");
                        Err(BuckyError::from(err))
                    }
                }
            });
            tasks.push(task);
        }

        futures::future::join_all(tasks).await
    }

    async fn request_inner(param: Vec<u8>, node: String) -> BuckyResult<Vec<u8>> {
        match async_std::future::timeout(Duration::new(20, 0), async move {
            let mut req = Request::new(Method::Post, Url::parse(format!("http://{}", node).as_str()).unwrap());
            req.insert_header(CONTENT_LENGTH, param.len().to_string());
            req.set_body(param);
            let stream = TcpStream::connect(node.as_str()).await.map_err(|err| {
                error!("connect to node failed! node={}, err={}", node.as_str(), err);
                BuckyError::from(err)
            })?;

            let mut resp = async_h1::connect(stream, req).await.map_err(|err| {
                error!("http connect error! node={}, err={}", node.as_str(), err);
                BuckyError::from(err)
            })?;

            if resp.status().is_success() {
                Ok(resp.body_bytes().await.map_err(|err| {
                    error!("http connect error! node={}, err={}", node.as_str(), err);
                    BuckyError::from(err)
                })?)
            } else {
                error!("error status:{}", resp.status());
                Err(crate::meta_err!(ERROR_NETWORK_ERROR))
            }
        }).await {
            Ok(ret) => {
                ret
            }
            Err(err) => {
                info!("connect timeout");
                Err(BuckyError::from(err))
            }
        }
    }

    async fn check_addr(node: String) -> BuckyResult<String> {
        let req = Request::new(Method::Get, Url::parse(format!("http://{}/check_addr", node).as_str()).unwrap());
        let stream = TcpStream::connect(node.as_str()).await.map_err(|err| {
            error!("connect to node failed! node={}, err={}", node.as_str(), err);
            BuckyError::from(err)
        })?;

        let mut resp = async_h1::connect(stream, req).await.map_err(|err| {
            error!("http connect error! node={}, err={}", node.as_str(), err);
            BuckyError::from(err)
        })?;

        if resp.status().is_success() {
            Ok(resp.body_string().await.unwrap())
        } else {
            error!("error status:{}", resp.status());
            Ok("".to_string())
        }
    }
}

#[async_trait]
impl ChainNetwork for HttpTcpChainNetwork {
    async fn broadcast(&self, obj: Vec<u8>) -> BuckyResult<()> {
        let mut count = 0;
        let mut node_list = {
            let list = self.node_list.lock().unwrap();
            let mut node_list = Vec::new();
            for (_, node) in list.iter() {
                node_list.push(node.clone());
            }
            node_list
        };
        while node_list.len() > 0 && count < 3 {
            let ret_list = HttpTcpChainNetwork::broadcast_inner(&obj, &node_list).await;
            let cur_node_list = node_list;
            node_list = Vec::new();
            let mut i = 0;
            for ret in ret_list {
                if let Err(_) = ret {
                    node_list.push(cur_node_list[i].clone());
                }
                i += 1;
            }
            count += 1;
        }

        Ok(())
    }

    async fn request(&self, param: Vec<u8>, to: Option<String>) -> BuckyResult<Vec<u8>> {
        if to.is_some() {
            HttpTcpChainNetwork::request_inner(param.clone(), to.unwrap()).await
        } else {
            let node_list = {
                let list = self.node_list.lock().unwrap();
                list.clone()
            };
            for _ in 0..node_list.len()*2 {
                let node = {
                    let mut rng = rand::thread_rng();
                    let (_, node) = &node_list[rng.gen_range(0..node_list.len())];
                    node.clone()
                };
                let ret = HttpTcpChainNetwork::request_inner(param.clone(), node).await;
                if ret.is_ok() {
                    return ret;
                }
            }
            Err(crate::meta_err!(ERROR_REQUEST_FAILED))
        }
    }

    async fn start(&self, ep: impl ChainNetworkEventEndpoint) -> BuckyResult<()> {
        let ep = Arc::new(ep);
        let addr = format!("0.0.0.0:{}", self.port);
        info!("bft server:{}", addr);
        let is_stop = self.is_stop.clone();
        async_std::task::spawn(async move {
            let listener_ret = TcpListener::bind(addr.as_str()).await;
            if let Ok(listener) = listener_ret {
                let tmp_is_stop = is_stop.clone();
                let mut incoming = listener.incoming();
                while let Some(stream) = incoming.next().await {
                    if *tmp_is_stop.lock().unwrap() {
                        break;
                    }
                    let stream = stream.unwrap();
                    let tmp_ep = ep.clone();
                    async_std::task::spawn(async move {
                        let tmp_ep = tmp_ep.clone();
                        let peer_addr = stream.peer_addr();
                        let ip = if peer_addr.is_ok() {
                            let ip = peer_addr.unwrap();
                            info!("starting new connection from {}", ip.clone());
                            ip.ip().to_string()
                        } else {
                            info!("new connection can't find remote addr.closed");
                            return;
                        };
                        let opts = async_h1::ServerOptions::default();
                        let ret = async_h1::accept_with_opts(stream, move |mut req: Request| {
                            let tmp_ep = tmp_ep.clone();
                            let ip = ip.clone();
                            async move {
                                let url = req.url();
                                let path = url.path();
                                // log::info!("url:{} path:{}", url.to_string(), path);
                                if path == "/check_addr" {
                                    let mut response = Response::new(StatusCode::Ok);
                                    response.insert_header(CONTENT_LENGTH, ip.len().to_string());
                                    response.set_body(ip.clone());
                                    return Ok(response);
                                }

                                let data = req.body_bytes().await?;
                                let ret = tmp_ep.call(data).await;
                                let res = if ret.is_ok() {
                                    let mut resp = Response::new(StatusCode::Ok);
                                    let data = ret.unwrap();
                                    resp.insert_header(CONTENT_LENGTH, data.len().to_string());
                                    resp.set_body(data);
                                    resp
                                } else {
                                    log::error!("handle err:{:?}", ret.err().unwrap());
                                    Response::new(StatusCode::Forbidden)
                                };
                                Ok(res)
                            }
                        }, opts)
                            .await;
                        if ret.is_err() {
                            log::error!("accept error:{}", ret.err().unwrap());
                        }
                    });
                }
            } else {
                error!("tcp listener bind failed.err:{}", addr)
            }
        });

        Ok(())
    }

    async fn stop(&self) -> BuckyResult<()> {
        *(self.is_stop.lock().unwrap()) = true;
        let addr = format!("127.0.0.1:{}", self.port);
        TcpStream::connect(addr.as_str()).await.unwrap();
        Ok(())
    }

    async fn has_connected(&self) -> BuckyResult<bool> {
        Ok(self.node_list.lock().unwrap().len() != 0)
    }

    async fn local_addr(&self) -> BuckyResult<String> {
        let mut address_info = HttpAddress {
            port: self.port,
            address_list: vec![]
        };
        if self.has_connected().await? {
            let node = {
                let node_list = self.node_list.lock().unwrap();
                node_list[0].1.clone()
            };
            let ip = HttpTcpChainNetwork::check_addr(node).await?;
            address_info.address_list.push(ip);
        } else {
            let ips = cyfs_util::get_all_ips()?;
            for ip in &ips {
                if ip.is_ipv4() {
                    address_info.address_list.push(ip.to_string());
                }
            }
        }
        Ok(serde_json::to_string(&address_info).unwrap())
    }

    async fn is_local_addr(&self, node: &str) -> BuckyResult<bool> {
        let ips = cyfs_util::get_all_ips()?;
        for ip in &ips {
            if ip.is_ipv4() && format!("{}:{}", ip.to_string(), self.port).as_str() == node {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn get_node_list(&self) -> BuckyResult<Vec<(String, String)>> {
        Ok(self.node_list.lock().unwrap().clone())
    }

    fn is_node_exist(&self, node: &str) -> BuckyResult<bool> {
        let node_list = self.node_list.lock().unwrap();
        for (_, inner_node) in node_list.iter() {
            if node == inner_node {
                return Ok(true);
            }
        }
        Ok(false)
    }

    async fn add_node(&self, node_id: &str, node: &str) -> BuckyResult<()> {
        let ret: Result<HttpAddress, _> = serde_json::from_str(node);
        if ret.is_err() {
            let mut node_list = self.node_list.lock().unwrap();
            let mut find = false;
            let mut i = 0;
            for (inner_id, inner) in node_list.iter() {
                if node_id == inner_id || node == inner {
                    find = true;
                    break;
                }
                i += 1;
            }
            if find {
                node_list.remove(i);
            }
            node_list.push((node_id.to_owned(), node.to_owned()));
        } else {
            let address_info = ret.unwrap();
            for address in &address_info.address_list {
                let node = format!("{}:{}", address, address_info.port);
                if self.is_node_exist(node.as_str())? {
                    return Ok(());
                }
            }
            for address in address_info.address_list {
                let node = format!("{}:{}", address, address_info.port);
                let ret = HttpTcpChainNetwork::check_addr(node.clone()).await;
                if ret.is_ok() {
                    let mut node_list = self.node_list.lock().unwrap();
                    let mut find = false;
                    let mut i = 0;
                    for (inner_id, inner_node) in node_list.iter() {
                        if inner_id == node_id || inner_node == node.as_str() {
                            find = true;
                            break;
                        }
                        i += 1;
                    }
                    if find {
                        node_list.remove(i);
                    }
                    node_list.push((node_id.to_owned(), node));
                    break;
                }
            }
        }
        Ok(())
    }

    fn get_node(&self, node_id: &str) -> Option<String> {
        let node_list = self.node_list.lock().unwrap();
        for (id, node) in node_list.iter() {
            if id == node_id {
                return Some(node.clone());
            }
        }
        None
    }
}

// #[cfg(test)]
// mod test_http_tcp_chain_network {
//     use crate::network::{HttpTcpChainNetwork, ChainNetwork};
//     use std::cell::RefCell;
//     use std::time::Duration;
//
//     async fn create_server(network_count: u8) -> Vec<RefCell<HttpTcpChainNetwork>> {
//         let mut list = Vec::new();
//         let mut port = 1345;
//         for _i in 0..network_count {
//             let network = RefCell::new(HttpTcpChainNetwork::new(port, Vec::new()));
//             list.push(network);
//             port += 1;
//         }
//
//         for network in &list {
//             let local_addr = network.borrow().local_addr().await.unwrap();
//             for tmp in &list {
//                 let tmp_addr = tmp.borrow().local_addr().await.unwrap();
//                 if tmp_addr != local_addr {
//                     network.borrow_mut().add_node(tmp_addr.as_str()).await.unwrap();
//                 }
//             }
//         }
//
//         return list;
//     }
//
//     #[test]
//     fn test_server() {
//         async_std::task::block_on(async {
//             let network_list = create_server(2).await;
//
//             network_list[0].borrow().start(Box::new(|_data| async move {
//                 Ok(Vec::new())
//             })).await.unwrap();
//
//             network_list[1].borrow().start(Box::new(|data: Vec<u8>| async move {
//                 assert_eq!(data.len(), 1);
//                 Ok(Vec::new())
//             })).await.unwrap();
//
//             async_std::task::sleep(Duration::new(1, 0)).await;
//
//             network_list[0].borrow().broadcast(vec![1]).await.unwrap();
//             async_std::task::sleep(Duration::new(1, 0)).await;
//
//             network_list[0].borrow_mut().stop().await.unwrap();
//             network_list[1].borrow_mut().stop().await.unwrap();
//
//             async_std::task::sleep(Duration::new(1, 0)).await;
//         })
//     }
// }

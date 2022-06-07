use super::http_bdt_listener::HttpBdtListenerManager;
use super::http_forward::HTTP_FORWARD_MANAGER;
use super::http_tcp_listener::HttpTcpListenerManager;
use cyfs_base::{BuckyError, BuckyErrorCode};

use std::collections::hash_map::{Entry, HashMap};

pub(super) struct HttpServer {
    id: String,
    forward_id: u32,
}

impl HttpServer {
    pub fn new(id: String, forward_id: u32) -> HttpServer {
        HttpServer { id, forward_id }
    }
}

pub struct HttpServerManager {
    bdt_listener_manager: HttpBdtListenerManager,
    tcp_listener_manager: HttpTcpListenerManager,

    named_server_list: HashMap<String, HttpServer>,
}

impl HttpServerManager {
    pub fn new() -> HttpServerManager {
        HttpServerManager {
            bdt_listener_manager: HttpBdtListenerManager::new(),
            tcp_listener_manager: HttpTcpListenerManager::new(),
            named_server_list: HashMap::new(),
        }
    }

    pub fn load(&mut self, stream_node: &Vec<toml::Value>) -> Result<(), BuckyError> {
        for v in stream_node {
            let node = v.as_table();
            if node.is_none() {
                error!("http server block is not object: {:?}", v);
                continue;
            }

            let node = node.unwrap();
            let block = node.get("block");
            if block.is_none() {
                error!("http server block miss block filed!{:?}", v);
                continue;
            }

            let block = block.unwrap().as_str().unwrap_or("server");
            match block {
                "server" => {
                    let _ret = self.load_server(node);
                }
                _ => {
                    warn!("unknown stream.block type: {}", block);
                }
            }
        }

        Ok(())
    }

    pub fn load_server(&mut self, server_node: &toml::value::Table) -> Result<(), BuckyError> {
        // 获取server id，可能为空
        let id = if let Some(v) = server_node.get("id") {
            v.as_str()
        } else {
            None
        };

        // 加载location相关
        let forward_id;
        {
            let ret = HTTP_FORWARD_MANAGER.lock().unwrap().load(&server_node);
            if ret.is_err() {
                return Err(ret.unwrap_err());
            }
            forward_id = ret.unwrap();
        }

        // 如果存在id字段，那么需要管理，并且必须唯一
        if id.is_some() {
            let id = id.unwrap();
            let server = HttpServer::new(id.to_owned(), forward_id);
            match self.named_server_list.entry(id.to_owned()) {
                Entry::Occupied(_v) => {
                    let msg = format!("http server with id already exists! id={}", id);
                    error!("{}", msg);

                    return Err(BuckyError::from((BuckyErrorCode::AlreadyExists, msg)));
                }
                Entry::Vacant(o) => o.insert(server),
            };
        }

        // 加载listener列表
        let listener_node = server_node.get("listener");
        if listener_node.is_none() {
            return BuckyError::error_with_log(format!("http block listener filed not found!"));
        }

        let listener_node = listener_node.unwrap();
        let ret;
        if listener_node.is_array() {
            ret = self.load_listeners(forward_id, listener_node.as_array().unwrap());
        } else if listener_node.is_table() {
            ret = self.load_listener(forward_id, listener_node.as_table().unwrap());
        } else {
            return BuckyError::error_with_log(
                format!("invalid http block listener format, array or object was expected")
                    .as_str(),
            );
        }

        if ret.is_err() {
            return ret;
        }

        Ok(())
    }

    fn load_listeners(
        &mut self,
        forward_id: u32,
        listener_list: &Vec<toml::Value>,
    ) -> Result<(), BuckyError> {
        for v in listener_list {
            if !v.is_table() {
                return BuckyError::error_with_log(format!(
                    "invalid http block listener format, array or object was expected"
                ));
            }

            let listener_node = v.as_table().unwrap();
            let ret = self.load_listener(forward_id, listener_node);
            if ret.is_err() {
                return ret;
            }
        }

        Ok(())
    }

    fn load_listener(
        &mut self,
        forward_id: u32,
        listener_node: &toml::value::Table,
    ) -> Result<(), BuckyError> {
        let mut listener_type = "tcp";
        let v = listener_node.get("type");
        if v.is_some() {
            listener_type = v.unwrap().as_str().unwrap();
        }

        match listener_type {
            "tcp" => {
                let ret = self.tcp_listener_manager.load(listener_node, forward_id);
                if ret.is_err() {
                    return Err(ret.unwrap_err());
                }
            }
            "bdt" => {
                let ret = self.bdt_listener_manager.load(listener_node);
                if ret.is_err() {
                    return Err(ret.unwrap_err());
                }

                let listener = ret.unwrap();
                let listener = listener.lock().unwrap();
                listener.bind_forward(forward_id);
            }
            _ => {
                let msg = format!("unknown http listener type: {:?}", v);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        Ok(())
    }

    pub fn start(&self) {
        self.tcp_listener_manager.start();
        self.bdt_listener_manager.start();
    }

    pub fn remove_server(&mut self, id: &str) -> Result<(), BuckyError> {
        let item = self.named_server_list.remove(id);
        if item.is_none() {
            let msg = format!("http server not found! id={}", id);
            error!("{}", msg);

            return Err(BuckyError::from((BuckyErrorCode::NotFound, msg)));
        }

        let server = item.unwrap();
        info!(
            "will stop and remove http server: id={}, forward={}",
            id, server.forward_id
        );

        self.bdt_listener_manager.unbind_forward(server.forward_id);
        self.tcp_listener_manager.unbind_forward(server.forward_id);

        Ok(())
    }
}

use crate::server::http::HttpServerManager;
use crate::server::stream::StreamServerManager;
use cyfs_base::{BuckyError, BuckyErrorCode};

use async_std::prelude::*;
use lru_time_cache::{Entry, LruCache};
use serde_json::Value;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone)]
struct DynamicServerInfo {
    owner: ControlServer,

    id: String,
    server_type: String,
    value: String,

    remove_on_drop: bool,
}

impl Drop for DynamicServerInfo {
    fn drop(&mut self) {
        if self.remove_on_drop {
            info!(
                "dync server gc without removed: id={}, type={}",
                self.id, self.server_type
            );
            let _r = self.owner.stop_server(self);
        }
    }
}

impl std::fmt::Display for DynamicServerInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "id={}, server_type={}, value={}",
            self.id, self.server_type, self.value
        )
    }
}
#[derive(Clone)]
pub(crate) struct ControlServer {
    list: Arc<Mutex<LruCache<String, DynamicServerInfo>>>,
    stream_server_manager: StreamServerManager,
    http_server_manager: HttpServerManager,
}

impl ControlServer {
    pub fn new(
        stream_server_manager: StreamServerManager,
        http_server_manager: HttpServerManager,
    ) -> Self {
        Self {
            stream_server_manager,
            http_server_manager,
            list: Arc::new(Mutex::new(LruCache::with_expiry_duration(
                Duration::from_secs(60),
            ))),
        }
    }

    pub fn start_monitor(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            let mut interval = async_std::stream::interval(Duration::from_secs(15));
            while let Some(_) = interval.next().await {
                this.gc();
            }
        });
    }

    fn gc(&self) {
        // 调用任何一个notify类api
        let (_, list) = self.list.lock().unwrap().notify_get("");

        for (_key, mut server) in list {
            info!(
                "will gc dync server: id={}, type={}",
                server.id, server.server_type
            );

            assert!(server.remove_on_drop);
            server.remove_on_drop = false;

            let _r = self.stop_server(&server);
        }

        let count = self.list.lock().unwrap().len();
        if count > 0 {
            debug!("dync server alive count={}", count);
        }
    }

    fn parse_server(&self, value: &str) -> Result<DynamicServerInfo, BuckyError> {
        let node: Value = ::serde_json::from_str(value).map_err(|e| {
            let msg = format!("load value as hjson error! value={}, err={}", value, e);
            error!("{}", msg);

            BuckyError::from((BuckyErrorCode::InvalidFormat, msg))
        })?;

        if !node.is_object() {
            let msg = format!("invalid value format, not object! value={}", value);
            error!("{}", msg);

            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        let node = node.as_object().unwrap();
        let id = if let Some(v) = node.get("id") {
            v.as_str()
        } else {
            None
        };

        let server_type = if let Some(v) = node.get("type") {
            v.as_str()
        } else {
            None
        };

        let block = if let Some(v) = node.get("value") {
            v.as_object()
        } else {
            None
        };

        if id.is_none() || server_type.is_none() || block.is_none() {
            let msg = format!(
                "invalid value format, id/type/value not found or invalid format! value={}",
                value
            );
            error!("{}", msg);

            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        let mut block = block.unwrap().clone();

        let id = id.unwrap();

        // 添加id到block里面
        if let Some(v) = block.insert("id".to_owned(), Value::String(id.to_owned())) {
            if !v.is_string() || v.as_str().unwrap() != id {
                let msg = format!(
                    "id and id in block dont match, id={}, id in block={}",
                    id, v
                );
                error!("{}", msg);

                return Err(BuckyError::from((BuckyErrorCode::Unmatch, msg)));
            }
        }

        let server = DynamicServerInfo {
            owner: self.clone(),
            id: id.to_owned(),
            server_type: server_type.unwrap().to_owned(),
            value: serde_json::to_string(&block).unwrap(),
            remove_on_drop: true,
        };

        Ok(server)
    }

    fn parse_unregister_server(&self, value: &str) -> Result<DynamicServerInfo, BuckyError> {
        let node: Value = ::serde_json::from_str(value).map_err(|e| {
            let msg = format!("load value as hjson error! value={}, err={}", value, e);
            error!("{}", msg);

            BuckyError::from((BuckyErrorCode::InvalidFormat, msg))
        })?;

        if !node.is_object() {
            let msg = format!("invalid value format, not object! value={}", value);
            error!("{}", msg);

            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        let node = node.as_object().unwrap();
        let id = if let Some(v) = node.get("id") {
            v.as_str()
        } else {
            None
        };

        let server_type = if let Some(v) = node.get("type") {
            v.as_str()
        } else {
            None
        };

        if id.is_none() || server_type.is_none() {
            let msg = format!(
                "invalid value format, id/type/value not found or invalid format! value={}",
                value
            );
            error!("{}", msg);

            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        let server = DynamicServerInfo {
            owner: self.clone(),
            id: id.unwrap().to_owned(),
            server_type: server_type.unwrap().to_owned(),
            value: "".to_owned(),
            remove_on_drop: true,
        };

        Ok(server)
    }

    /*
    {
        "id": "{id}",
        "type": "http|stream",
        "value": {

        }
    }
    */
    pub fn register_server(&self, value: &str) -> Result<(), BuckyError> {
        let mut server = self.parse_server(value)?;

        // 检查是否已经存在，存在的话比较是否发生改变
        let full_id = format!("{}_{}", server.server_type, server.id);

        // entry调用会默认移除超时对象，所以这里先显式的调用一次gc
        self.gc();

        let mut server = match self.list.lock().unwrap().entry(full_id) {
            Entry::Occupied(oc) => {
                // server会被立即drop，但由于同id的server还有效，所以不能触发remove
                server.remove_on_drop = false;

                let cur = oc.into_mut();
                if cur.value == server.value {
                    debug!(
                        "dynamic server already exists! id={}, type={}",
                        server.id, server.server_type
                    );

                    return Ok(());
                } else {
                    info!(
                        "dynamic server exists but value changed! id={}, type={}, old={}, new={}",
                        server.id, server.server_type, cur.value, server.value,
                    );

                    // 先停止现有服务
                    let _r = self.stop_server(&cur);

                    cur.value = server.value.clone();
                }

                cur.to_owned()
            }
            Entry::Vacant(vc) => {
                info!(
                    "first add dynamic server! id={}, type={}, value={}",
                    server.id, server.server_type, server.value,
                );

                vc.insert(server).to_owned()
            }
        };

        server.remove_on_drop = false;
        self.add_server(&server)?;

        Ok(())
    }

    /*
    {
        "id": "{id}",
        "type": "http|stream",
    }
    */
    pub fn unregister_server(&self, value: &str) -> Result<(), BuckyError> {
        let server = self.parse_unregister_server(value)?;

        let full_id = format!("{}_{}", server.server_type, server.id);
        let item = self.list.lock().unwrap().remove(&full_id);
        match item {
            Some(mut v) => {
                info!("server removed! id={}, type={}", v.id, v.server_type);
                v.remove_on_drop = false;
                self.stop_server(&v)
            }
            None => {
                let msg = format!(
                    "server not found! id={}, type={}",
                    server.id, server.server_type
                );
                error!("{}", msg);

                Err(BuckyError::from((BuckyErrorCode::NotFound, msg)))
            }
        }
    }

    fn add_server(&self, server: &DynamicServerInfo) -> Result<(), BuckyError> {
        match server.server_type.as_str() {
            "http" => {
                let value: toml::Value = serde_json::from_str(&server.value).unwrap();

                self.http_server_manager
                    .load_server(value.as_table().unwrap())?;

                self.http_server_manager.start();
            }
            "stream" => {
                let value: toml::Value = serde_json::from_str(&server.value).unwrap();
                self.stream_server_manager
                    .load_server(value.as_table().unwrap())?;

                self.stream_server_manager.start();
            }
            value @ _ => {
                let msg = format!(
                    "invalid gateway server type, only http/stream accept! type={}",
                    value
                );
                error!("{}", msg);

                return Err(BuckyError::from((BuckyErrorCode::UnSupport, msg)));
            }
        }

        Ok(())
    }

    fn stop_server(&self, server: &DynamicServerInfo) -> Result<(), BuckyError> {
        match server.server_type.as_str() {
            "http" => {
                self.http_server_manager.remove_server(&server.id)?;
            }
            "stream" => {
                self.stream_server_manager.remove_server(&server.id)?;
            }
            value @ _ => {
                let msg = format!(
                    "invalid gateway server type, only http/stream accept! type={}",
                    value
                );
                error!("{}", msg);

                return Err(BuckyError::from((BuckyErrorCode::UnSupport, msg)));
            }
        }

        Ok(())
    }
}

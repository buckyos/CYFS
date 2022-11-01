use crate::gateway::GATEWAY;
use cyfs_base::{BuckyError, BuckyErrorCode};

use async_std::prelude::*;
use lazy_static::lazy_static;
use lru_time_cache::{Entry, LruCache};
use serde_json::{Value};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug)]
struct DynamicServerInfo {
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
            let _r = ControlServerInner::stop_server(self);
        }
    }
}

pub(crate) struct ControlServerInner {
    list: LruCache<String, DynamicServerInfo>,
}

impl ControlServerInner {
    pub fn new() -> ControlServerInner {
        ControlServerInner {
            list: LruCache::with_expiry_duration(Duration::from_secs(60)),
        }
    }

    fn gc(&mut self) {
        // 调用任何一个notify类api
        let (_, list) = self.list.notify_get("");

        for (_key, mut server) in list {
            info!(
                "will gc dync server: id={}, type={}",
                server.id, server.server_type
            );

            assert!(server.remove_on_drop);
            server.remove_on_drop = false;

            let _r = Self::stop_server(&server);
        }

        if self.list.len() > 0 {
            debug!("dync server alive count={}", self.list.len());
        }
    }

    fn parse_server(value: &str) -> Result<DynamicServerInfo, BuckyError> {
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
            id: id.to_owned(),
            server_type: server_type.unwrap().to_owned(),
            value: serde_json::to_string(&block).unwrap(),
            remove_on_drop: true,
        };

        Ok(server)
    }

    fn parse_unregister_server(value: &str) -> Result<DynamicServerInfo, BuckyError> {
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
    pub fn register_server(&mut self, value: &str) -> Result<(), BuckyError> {
        let mut server = Self::parse_server(value)?;

        // 检查是否已经存在，存在的话比较是否发生改变
        let full_id = format!("{}_{}", server.server_type, server.id);

        // entry调用会默认移除超时对象，所以这里先显式的调用一次gc
        self.gc();

        let server = match self.list.entry(full_id) {
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
                    let _r = Self::stop_server(&cur);

                    cur.value = server.value.clone();
                }

                cur
            }
            Entry::Vacant(vc) => {
                info!(
                    "first add dynamic server! id={}, type={}, value={}",
                    server.id, server.server_type, server.value,
                );

                vc.insert(server)
            }
        };

        Self::add_server(server)?;

        Ok(())
    }

    /*
    {
        "id": "{id}",
        "type": "http|stream",
    }
    */
    pub fn unregister_server(&mut self, value: &str) -> Result<(), BuckyError> {
        let server = Self::parse_unregister_server(value)?;

        let full_id = format!("{}_{}", server.server_type, server.id);
        match self.list.remove(&full_id) {
            Some(mut v) => {
                info!("server removed! id={}, type={}", v.id, v.server_type);
                v.remove_on_drop = false;
                Self::stop_server(&v)
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

    fn add_server(
        server: &DynamicServerInfo,
    ) -> Result<(), BuckyError> {
        match server.server_type.as_str() {
            "http" => {
                let value: toml::Value = serde_json::from_str(&server.value).unwrap();
                
                GATEWAY.http_server_manager.load_server(value.as_table().unwrap())?;

                GATEWAY.http_server_manager.start();
            }
            "stream" => {
                let value: toml::Value = serde_json::from_str(&server.value).unwrap();
                GATEWAY.stream_server_manager.load_server(value.as_table().unwrap())?;

                GATEWAY.stream_server_manager.start();
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

    fn stop_server(server: &DynamicServerInfo) -> Result<(), BuckyError> {
        match server.server_type.as_str() {
            "http" => {
                GATEWAY.http_server_manager.remove_server(&server.id)?;
            }
            "stream" => {
                GATEWAY.stream_server_manager.remove_server(&server.id)?;
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

lazy_static! {
    static ref CONTROL_SERVER: Mutex<ControlServerInner> = {
        return Mutex::new(ControlServerInner::new());
    };
}

pub(crate) struct ControlServer;

impl ControlServer {
    pub fn new() -> ControlServer {
        ControlServer {}
    }

    pub fn register_server(value: &str) -> Result<(), BuckyError> {
        CONTROL_SERVER.lock().unwrap().register_server(value)
    }

    pub fn unregister_server(value: &str) -> Result<(), BuckyError> {
        CONTROL_SERVER.lock().unwrap().unregister_server(value)
    }

    pub fn start_monitor() {
        async_std::task::spawn(async move {
            let mut interval = async_std::stream::interval(Duration::from_secs(15));
            while let Some(_) = interval.next().await {
                let mut control_server = CONTROL_SERVER.lock().unwrap();
                control_server.gc();
            }
        });
    }
}

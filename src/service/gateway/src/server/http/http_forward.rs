use http_types::{headers::HOST, Request};
use std::sync::{Arc, Mutex};

use super::http_location::HttpLocationManager;
use super::server_name::ServerName;
use base::VAR_MANAGER;
use cyfs_base::BuckyError;

pub(super) struct HttpForward {
    id: u32,
    server_name: Vec<ServerName>,
    location: Arc<Mutex<HttpLocationManager>>,
}

pub(super) struct HttpForwardManager {
    next_id: u32,
    forward_list: std::collections::HashMap<u32, Arc<Mutex<HttpForward>>>,
}

impl HttpForward {
    pub fn new(id: u32) -> HttpForward {
        HttpForward {
            id,
            server_name: Vec::new(),
            location: Arc::new(Mutex::new(HttpLocationManager::new())),
        }
    }

    /*
    pub fn id(&self) -> u32 {
        return self.id;
    }
    */

    pub fn load(
        &mut self,
        server_node: &toml::value::Table,
    ) -> Result<(), BuckyError> {
        for (k, v) in server_node {
            match k.as_str() {
                "server_name" => {
                    // server_name 字段需要翻译
                    let tv = VAR_MANAGER
                        .translate_str(v.as_str().unwrap_or(""))?;

                    let list: Vec<&str> = tv.split(' ').collect();
                    self.server_name = list
                        .iter()
                        .filter_map(|&value| match ServerName::parse(value) {
                            Ok(v) => Some(v),
                            Err(_e) => None,
                        })
                        .collect();
                }
                "location" => {
                    if v.is_array() {
                        let mut mgr = self.location.lock().unwrap();
                        mgr.load(&v.as_array().unwrap())?;
                    } else {
                        let msg = format!("config invalid http location format");
                        error!("{}", msg);

                        return Err(BuckyError::from(msg));
                    }
                }
                _ => {
                    // warn!("unknown server block field: {}", k);
                }
            }
        }

        Ok(())
    }

    fn check_host(&self, req: &Request) -> Option<()> {
        // 判断server是否匹配
        let host = req.header(&HOST);

        let hostname = match host {
            Some(v) => {
                let host = format!("http://{}", v.last().as_str());
                match ::url::Url::parse(&host) {
                    Ok(v) => v.host_str().map(|v| v.to_owned()),
                    Err(e) => {
                        error!("parse host header error! err={}", e);
                        None
                    }
                }
            }
            None => {
                // req.url().host_str()
                // FIXME 是不是只使用HOST header？
                None
            }
        };

        for server_name in &self.server_name {
            if server_name.is_match(hostname.as_deref()) {
                return Some(());
            }
        }

        warn!(
            "unmatch server name, accpet={:?}, request={:?}",
            self.server_name, hostname
        );
        return None;
    }

    pub fn find_dispatch(&self, req: &Request) -> Option<String> {
        // 判断server是否匹配
        if self.check_host(&req).is_none() {
            return None;
        }

        // 获取path
        let path = req.url().path();
        info!("req path is {}", path);

        // 查找对应的location

        let item = self.location.lock().unwrap().search(&req);
        if item.is_none() {
            error!("location not matched, path={}", path);

            return None;
        }

        Some(item.unwrap().to_owned())
    }
}

impl HttpForwardManager {
    pub fn new() -> HttpForwardManager {
        HttpForwardManager {
            next_id: 1,
            forward_list: std::collections::HashMap::new(),
        }
    }

    pub fn load(&mut self, location_node: &toml::value::Table) -> Result<u32, BuckyError> {
        let forward_id = self.next_id;
        let mut forward = HttpForward::new(forward_id);
        if let Err(e) = forward.load(location_node) {
            return Err(e);
        }

        self.next_id += 1;
        self.forward_list
            .insert(forward_id, Arc::new(Mutex::new(forward)));

        return Ok(forward_id);
    }

    pub fn get_forward(&self, id: &u32) -> Option<Arc<Mutex<HttpForward>>> {
        if let Some(server) = self.forward_list.get(id) {
            return Some(server.clone());
        }

        return None;
    }
}

use lazy_static::lazy_static;

lazy_static! {
    pub(super) static ref HTTP_FORWARD_MANAGER: Mutex<HttpForwardManager> = {
        return Mutex::new(HttpForwardManager::new());
    };
}

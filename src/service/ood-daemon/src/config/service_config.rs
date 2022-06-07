use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
pub enum ServiceState {
    RUN,
    STOP,
}

impl fmt::Display for ServiceState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let state = match &self {
            ServiceState::RUN => "run",
            ServiceState::STOP => "stop",
        };

        write!(f, "{}", state)
    }
}

impl FromStr for ServiceState {
    type Err = BuckyError;

    fn from_str(s: &str) -> BuckyResult<Self> {
        match s.to_lowercase().as_str() {
            "run" => Ok(ServiceState::RUN),
            "stop" => Ok(ServiceState::STOP),
            _ => {
                let msg = format!("unknown service state: {}", s);
                error!("{}", msg);

                Err(BuckyError::new(BuckyErrorCode::NotSupport, msg))
            }
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    // service对象id
    pub id: String,

    pub name: String,
    pub fid: String,
    pub version: String,
    pub enable: bool,
    pub target_state: ServiceState,
}

impl ServiceConfig {
    pub fn new() -> Self {
        Self {
            id: String::from(""),
            name: String::from(""),
            fid: String::from(""),
            version: String::from(""),
            enable: false,
            target_state: ServiceState::STOP,
        }
    }

    pub fn update(&mut self, target: &ServiceConfig) {
        self.id = target.id.clone();
        self.name = target.name.clone();
        self.fid = target.fid.clone();
        self.version = target.version.clone();
        self.enable = target.enable;
        self.target_state = target.target_state;
    }

    pub fn load_service_list(service_list: &Vec<toml::Value>) -> BuckyResult<Vec<Self>> {
        let mut list: Vec<Self> = Vec::new();

        for v in service_list {
            let service_node = v.as_table();
            if service_node.is_none() {
                error!("service block is not object: {:?}", v);
                continue;
            }

            let service_node = service_node.unwrap();
            let mut service_info = Self::new();
            let ret = service_info.load(service_node);
            if ret.is_ok() {
                list.push(service_info);
            } else {
                // 加载service失败，这里先忽略
            }
        }

        Ok(list)
    }

    fn load(&mut self, service_node: &toml::value::Table) -> BuckyResult<()> {
        assert!(self.name.is_empty());
        assert!(self.fid.is_empty());

        for (k, v) in service_node {
            match k.as_str() {
                "id" => {
                    if !v.is_str() {
                        error!("invalid service id field type: {:?}", v);
                    }

                    self.id = v.as_str().unwrap_or("").to_owned();
                }
                "name" => {
                    if !v.is_str() {
                        error!("invalid service name field type: {:?}", v);
                    }

                    self.name = v.as_str().unwrap_or("").to_owned();
                }
                "fid" => {
                    if !v.is_str() {
                        error!("invalid service fid field type: {:?}", v);
                    }

                    self.fid = v.as_str().unwrap_or("").to_owned();
                }
                "version" => {
                    if !v.is_str() {
                        error!("invalid service version field type: {:?}", v);
                    }

                    self.version = v.as_str().unwrap_or("").to_owned();
                }
                "enable" => {
                    if !v.is_bool() {
                        error!("invalid service enable field type: {:?}", v);
                    }

                    self.enable = v.as_bool().unwrap_or(false);
                }
                "target_state" => {
                    if !v.is_str() {
                        error!("invalid service target_state field type: {:?}", v);
                    }

                    let v = v.as_str().unwrap_or("stop");
                    match ServiceState::from_str(v) {
                        Ok(state) => self.target_state = state,
                        Err(_) => {
                            self.target_state = ServiceState::STOP;
                        }
                    }
                }
                _ => {}
            }
        }

        if self.name.is_empty() || self.fid.is_empty() {
            let msg = format!(
                "load service config failed! name or fid not specified! {:?}",
                self
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        Ok(())
    }
}

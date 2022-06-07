use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_debug::Mutex;

use lazy_static::lazy_static;
use regex::{Captures, Regex};
use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc};

pub struct Var {
    name: String,
    value: String,
}

impl fmt::Display for Var {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.name, self.value)
    }
}

struct VarManagerImpl {
    all: HashMap<String, Var>,
}

lazy_static! {
    static ref VAR_REGEX: Regex = Regex::new(r"\$[\w]+").unwrap();
}

impl VarManagerImpl {
    pub fn new() -> VarManagerImpl {
        VarManagerImpl {
            all: HashMap::new(),
        }
    }

    pub fn add(&mut self, key: String, value: String) {
        info!("add new var: {} = {}", key, value);

        if let Some(v) = self.all.insert(key.clone(), Var { name: key, value }) {
            warn!("replace var: old={}", v)
        }
    }

    pub fn init(&mut self) -> BuckyResult<()> {
        if let Err(e) = self.init_hosts() {
            error!("init hosts error! {}", e);
        }

        info!("init default vars as follows:");
        for (_, v) in &self.all {
            info!("{}", v);
        }

        Ok(())
    }

    pub fn translate_str(&self, value: &str) -> BuckyResult<String> {
        let mut err: Option<String> = None;
        let result = VAR_REGEX.replace_all(value, |v: &Captures| match self.all.get(&v[0]) {
            Some(var) => var.value.clone(),
            None => {
                let msg = format!("var not found! {}", &v[0]);
                warn!("{}", msg);
                err = Some(msg.to_owned());

                v[0].to_string()
            }
        });

        if let Some(msg) = err {
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        } else {
            Ok(result.to_string())
        }
    }

    // 诸如此类 ${local_ip_v4}:1020 -> 192.168.100.100:1020
    // 如果local_ip_v4对应多个值，那么只取第一个
    pub fn translate_addr_str(&self, value: &str) -> BuckyResult<String> {
        let mut err: Option<String> = None;
        let result = VAR_REGEX.replace(value, |v: &Captures| match self.all.get(&v[0]) {
            Some(var) => var.value.split(" ").next().unwrap().to_owned(),
            None => {
                let msg = format!("var not found! {}", &v[0]);
                warn!("{}", msg);
                err = Some(msg.to_owned());

                v[0].to_string()
            }
        });

        if let Some(msg) = err {
            Err(BuckyError::new(BuckyErrorCode::NotFound, msg))
        } else {
            debug!("{} -> {}", value, result);
            Ok(result.to_string())
        }
    }

    fn init_hosts(&mut self) -> BuckyResult<()> {
        // 添加本地回环地址
        self.all.insert(
            "$local_ip_v4".to_owned(),
            Var {
                name: "$local_ip_v4".to_owned(),
                value: "127.0.0.1".to_owned(),
            },
        );

        let addr_list = cyfs_util::get_system_hosts()?;
        let none_local_ip_v4: Vec<String> = addr_list
            .none_local_ip_v4
            .iter()
            .map(|v| v.ip().to_string())
            .collect();
        self.all.insert(
            "$none_local_ip_v4".to_owned(),
            Var {
                name: "$none_local_ip_v4".to_owned(),
                value: none_local_ip_v4.join(" "),
            },
        );

        let private_ip_v4: Vec<String> = addr_list
            .private_ip_v4
            .iter()
            .map(|v| v.ip().to_string())
            .collect();
        self.all.insert(
            "$private_ip_v4".to_owned(),
            Var {
                name: "$private_ip_v4".to_owned(),
                value: private_ip_v4.join(" "),
            },
        );

        let public_ip_v4: Vec<String> = addr_list
            .public_ip_v4
            .iter()
            .map(|v| v.ip().to_string())
            .collect();
        self.all.insert(
            "$public_ip_v4".to_owned(),
            Var {
                name: "$public_ip_v4".to_owned(),
                value: public_ip_v4.join(" "),
            },
        );

        let ip_v6: Vec<String> = addr_list.ip_v6.iter().map(|v| v.to_string()).collect();
        self.all.insert(
            "$ip_v6".to_owned(),
            Var {
                name: "$ip_v6".to_owned(),
                value: ip_v6.join(" "),
            },
        );

        Ok(())
    }
}

pub struct VarManager(Arc<Mutex<VarManagerImpl>>);

impl VarManager {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(VarManagerImpl::new())))
    }

    pub fn add(&self, key: String, value: String) {
        self.0.lock().unwrap().add(key, value)
    }

    pub fn init(&self) -> BuckyResult<()> {
        self.0.lock().unwrap().init()
    }

    pub fn translate_str(&self, value: &str) -> BuckyResult<String> {
        self.0.lock().unwrap().translate_str(value)
    }

    pub fn translate_addr_str(&self, value: &str) -> BuckyResult<String> {
        self.0.lock().unwrap().translate_addr_str(value)
    }

    // 替换变量，并解析为host列表
    pub fn replace_socket_addr_and_parse(&self, value: &str) -> BuckyResult<Vec<SocketAddr>> {
        let tv = VAR_MANAGER.translate_str(value)?;
        let list: Vec<&str> = tv.split(" ").collect();
        let mut addr_list = Vec::new();

        // 替换后，可能存在多个ip
        for item in list {
            if item.is_empty() {
                // 替换的变量为空，那么可能造成空字符串，需要忽略
                continue;
            }

            addr_list.push(VAR_MANAGER.parse_as_socket_addr(item)?);
        }

        Ok(addr_list)
    }

    pub fn parse_as_socket_addr(&self, value: &str) -> BuckyResult<SocketAddr> {
        let ret;
        match value.parse::<IpAddr>() {
            Ok(v) => {
                ret = SocketAddr::new(v, 0);
            }
            Err(_e) => {
                // ipv6的由于存在scope_id，所以是SocketAddr的格式存在
                match value.parse::<SocketAddr>() {
                    Ok(addr) => {
                        ret = addr;
                    }
                    Err(e) => {
                        error!("parse host as socket addr error! host={}, e={}", value, e);
                        return Err(BuckyError::from(e));
                    }
                }
            }
        };

        Ok(ret)
    }
}

lazy_static! {
    pub static ref VAR_MANAGER: VarManager = VarManager::new();
}

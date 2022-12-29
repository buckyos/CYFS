use crate::VAR_MANAGER;
use cyfs_base::*;
use cyfs_util::TomlHelper;

use std::str::FromStr;

// tcp端口映射环境变量
const TCP_PORT_MAPPING_KEY: &str = "CYFS_TCP_PORT_MAPPING";

// 外部指定的bdt_port变量
const BDT_PORT_KEY: &str = "CYFS_BDT_PORT";

// udp_sn_only
const UDP_SN_ONLY_KEY: &str = "CYFS_UDP_SN_ONLY";

// bdt层的配置参数
pub(crate) struct BdtParams {
    pub device: String,

    pub endpoint: Vec<Endpoint>,

    // tcp端口映射，如果存在的话
    pub tcp_port_mapping: Option<u16>,

    // disable udp transport but sn online via udp
    pub udp_sn_only: Option<bool>,
}

impl Default for BdtParams {
    fn default() -> Self {
        Self {
            // 默认使用etc/desc/device.desc && device.sec
            device: "device".to_owned(),
            endpoint: vec![],
            tcp_port_mapping: None,
            udp_sn_only: None,
        }
    }
}

pub(crate) struct BdtConfigLoader {
    params: BdtParams,
}

impl Into<BdtParams> for BdtConfigLoader {
    fn into(self) -> BdtParams {
        self.params
    }
}

impl BdtConfigLoader {
    pub fn new(params: BdtParams) -> Self {
        Self { params }
    }

    pub fn load(&mut self, node: &toml::value::Table) -> BuckyResult<()> {
        for (k, v) in node {
            match k.as_str() {
                "config" => {
                    if !v.is_table() {
                        let msg = format!("invalid non stack.bdt.config field format: {:?}", v);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }

                    self.load_config(v.as_table().unwrap())?;
                }

                "endpoint" => {
                    assert!(v.is_array());
                    if v.is_array() {
                        self.params.endpoint = Self::load_endpoints(&v.as_array().unwrap())
                            .map_err(|e| {
                                error!("load stack.bdt.endpoint error! {}", e);
                                e
                            })?;
                    } else if v.is_table() {
                        self.params.endpoint = Self::load_endpoint(&v.as_table().unwrap())
                            .map_err(|e| {
                                error!("load stack.bdt.endpoint error! {}", e);
                                e
                            })?;
                    } else {
                        let msg = format!("invalid stack.bdt.endpoint format! {:?}", v);
                        error!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                }
                _ => {
                    warn!("unknown stack.bdt field: {}", k.as_str());
                }
            }
        }

        // 环境变量优先级大于静态配置
        self.load_tcp_port_mapping_from_env();

        Ok(())
    }

    fn load_config(&mut self, node: &toml::value::Table) -> BuckyResult<()> {
        for (k, v) in node {
            match k.as_str() {
                "desc" | "device" => {
                    self.params.device = TomlHelper::decode_from_string(v)?;
                }
                "tcp_port_mapping" => {
                    if v.is_integer() {
                        let port = v.as_integer().unwrap() as u16;
                        self.add_tcp_port_mapping(port);
                    } else if v.is_str() {
                        self.add_tcp_port_mapping_str(v.as_str().unwrap());
                    } else {
                        let msg = format!(
                            "invalid tcp_port_mapping field, except int or string: {}",
                            v
                        );
                        warn!("{}", msg);
                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }
                }
                "udp_sn_only" => {
                    self.params.udp_sn_only = Some(TomlHelper::decode_from_boolean(v)?);
                }
                _ => {
                    warn!("unknown stack.bdt.config field: {}", k.as_str());
                }
            }
        }

        if let Some(udp_sn_only) = Self::load_udp_sn_only_from_env() {
            self.params.udp_sn_only = Some(udp_sn_only);
        }

        Ok(())
    }

    pub(crate) fn load_endpoints(endpoints_node: &Vec<toml::Value>) -> BuckyResult<Vec<Endpoint>> {
        let mut ret = Vec::new();
        for v in endpoints_node {
            if !v.is_table() {
                let msg = format!("invalid bdt endpoint node format! node={:?}", v);
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }

            let node = v.as_table().unwrap();
            let mut endpoints = Self::load_endpoint(&node)?;
            if !endpoints.is_empty() {
                ret.append(&mut endpoints);
            }
        }

        if ret.is_empty() {
            let msg = format!("no valid endpoint found!");
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        Ok(ret)
    }

    fn load_endpoint(endpoint_node: &toml::value::Table) -> BuckyResult<Vec<Endpoint>> {
        let mut addr_list = Vec::new();
        let mut port: u16 = 0;
        let mut protocol: Option<Protocol> = None;
        let mut optional = false;
        let mut system_default = false;

        for (k, v) in endpoint_node {
            match k.as_str() {
                // 读取是不是可选,可选的情况下，转换变量失败会直接忽略该地址
                "optional" => optional = v.as_bool().unwrap_or(false),
                "host" => {
                    let host_str = v.as_str().unwrap_or("");
                    addr_list = VAR_MANAGER.replace_socket_addr_and_parse(host_str)?;
                }
                "port" => match cyfs_util::parse_port_from_toml_value(v) {
                    Ok(p) => {
                        port = p;
                    }
                    Err(e) => {
                        return Err(e);
                    }
                },
                "protocol" => {
                    let protocol_str = v.as_str().unwrap_or("").to_lowercase();
                    match protocol_str.as_str() {
                        "tcp" => protocol = Some(Protocol::Tcp),
                        "udp" => protocol = Some(Protocol::Udp),
                        _ => {
                            let msg = format!("bdt protocol not support: {}", protocol_str);
                            error!("{}", msg);
                            return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
                        }
                    }
                }
                "system_default" => system_default = v.as_bool().unwrap_or(false),
                _ => {
                    warn!("unknown bdt endpoint field: {}", k.as_str());
                }
            }
        }

        if addr_list.is_empty() {
            if optional {
                warn!(
                    "translate endpoint host but not found, will ignore as optional: endpoint={:?}",
                    endpoint_node
                );
                return Ok(Vec::new());
            } else {
                let msg = format!(
                    "translate endpoint host error: endpoint={:?}",
                    endpoint_node
                );
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
            }
        }

        // 查看外部有没有通过环境变量指定bdt_port
        if let Some(env_port) = Self::load_bdt_port_from_env() {
            port = env_port;
        }

        if protocol.is_none() {
            let msg = format!("invalid bdt endpoint fields! node={:?}", endpoint_node);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        let mut have_ipv6 = false;
        let mut endpoints = Vec::new();
        for mut item in addr_list {
            // ipv6不绑定具体地址，这里先设置一个标志位
            if item.is_ipv6() {
                have_ipv6 = true;
                continue;
            }

            item.set_port(port);

            let mut endpoint = Endpoint::from((protocol.unwrap(), item));
            // 如果设置了system_default标志，将endpoint的system_default设置为true，这个选项在移动平台使用
            if system_default {
                endpoint.set_area(EndpointArea::Default);
            }
            endpoints.push(endpoint);
        }

        // 如果有ipv6地址，就设置一个ipv6的0地址到endpoint
        if have_ipv6 {
            let addr = SocketAddr::new(IpAddr::from_str("::").unwrap(), port);
            let mut endpoint = Endpoint::from((protocol.unwrap(), addr));
            // 如果设置了system_default标志，将endpoint的system_default设置为true，这个选项在移动平台使用
            if system_default {
                endpoint.set_area(EndpointArea::Default);
            }
            endpoints.push(endpoint);
        }

        Ok(endpoints)
    }

    fn add_tcp_port_mapping_str(&mut self, val: &str) {
        for val in val.split(',') {
            match val.parse::<u16>() {
                Ok(port) => {
                    self.add_tcp_port_mapping(port);
                }
                Err(e) => {
                    error!("invalid tcp port mapping value! val={}, {}", val, e);
                }
            }
        }
    }

    fn add_tcp_port_mapping(&mut self, port: u16) {
        // 目前只支持一个tcp端口映射
        if let Some(old) = &self.params.tcp_port_mapping {
            if *old != port {
                error!(
                    "only one tcp port mapping support! old={}, new={}",
                    old, port
                );
            } else {
                return;
            }
        }

        info!("will add tcp port mapping: {}", port);
        self.params.tcp_port_mapping = Some(port);
    }

    fn load_tcp_port_mapping_from_env(&mut self) {
        match std::env::var(TCP_PORT_MAPPING_KEY) {
            Ok(val) => {
                info!(
                    "got static tcp port from env var: env={}, val={}",
                    TCP_PORT_MAPPING_KEY, val
                );
                self.add_tcp_port_mapping_str(&val);
            }
            Err(_) => {}
        };
    }

    fn load_bdt_port_from_env() -> Option<u16> {
        match std::env::var(BDT_PORT_KEY) {
            Ok(val) => {
                info!(
                    "got bdt port from env var: env={}, val={}",
                    BDT_PORT_KEY, val
                );

                match val.parse::<u16>() {
                    Ok(v) => Some(v),
                    Err(e) => {
                        error!("invalid port number! val={}, {}", val, e);

                        None
                    }
                }
            }
            Err(_) => None,
        }
    }

    fn load_udp_sn_only_from_env() -> Option<bool> {
        match std::env::var(UDP_SN_ONLY_KEY) {
            Ok(val) => {
                info!(
                    "got udp_sn_only from env var: env={}, val={}",
                    UDP_SN_ONLY_KEY, val
                );

                let value = val.trim();
                if value == "true" || value == "1" {
                    Some(true)
                } else {
                    Some(false)
                }
            }
            Err(_) => None,
        }
    }
}

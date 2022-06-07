use crate::{DeviceInfo, LOCAL_DEVICE_MANAGER};
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};

pub const BDT_ENDPOINTS: &str = r#"
[[stack.bdt.endpoint]]
optional = true
host = "$none_local_ip_v4"
port = ${bdt_port}
protocol = "tcp"
system_default = ${system_default}

[[stack.bdt.endpoint]]
optional = true
host = "$none_local_ip_v4"
port = ${bdt_port}
protocol = "udp"
system_default = ${system_default}

[[stack.bdt.endpoint]]
optional = true
host = "$ip_v6"
port = ${bdt_port}
protocol = "tcp"
system_default = ${system_default}

[[stack.bdt.endpoint]]
optional = true
host = "$ip_v6"
port = ${bdt_port}
protocol = "udp"
system_default = ${system_default}
"#;

const BDT_CONFIG: &str = r#"
[stack.bdt.config]
#tcp_port_mapping = 0
device = "${device_file_name}"
#udp_sn_only = false

${endpoints}
"#;

const NON_CONFIG: &str = r#"
[stack.config]
id = "${id}"
shared_stack = ${shared_stack}
shared_stack_stub = ${shared_stack_stub}
sync_service = ${sync_service}
isolate = "${isolate}"

[stack.meta]
#target = dev

[stack.noc]
type = "sqlite"

[[stack.interface]]
type = "http"
listen = "${http_listener}"

[[stack.interface]]
type = "http-bdt"
vport = "${http-bdt-vport}"

[[stack.interface]]
type = "ws"
enable = ${ws_enable}
listen = "${ws_listener}"

# bdt配置部分，可替换默认值
${bdt_config}
"#;

// 用来生成bdt endpoints的配置
pub struct BdtEndPointParams {
    pub none_local_ip_v4: Option<String>,
    pub ip_v6: Option<String>,

    // bdt协议栈的本地地址
    pub bdt_port: u16,

    // 是不是移动端
    pub is_mobile_stack: bool,
}

// FIXME 以后需要随机化端口
// FIXME bdt协议栈也需要端口随机化
pub struct CyfsServiceLoaderParam {
    // 协议栈id，默认为default，如果需要使用多个协议栈，那么需要指定不同的id
    pub id: Option<String>,

    // 配置和数据库的隔离，如果/etc, /cyfs/data目录需要和其余的dec app共享，那么这里需要指定隔离目录，否则会导致冲突
    pub isolate: Option<String>,

    // non-service本地http服务的地址
    pub non_http_addr: String,

    // non-stack的ws服务的地址，可以选择是否开启
    // 默认开启
    pub non_ws_addr: Option<String>,

    // bdt协议栈的本地地址
    pub bdt_port: u16,

    // 配置独立的bdt endpoints
    pub bdt_endpoints: Option<String>,

    // 指定device在/cyfs/etc/desc的文件名字
    // 如果需要直接指定一个内存中的device，那么需要同时传入device字段
    pub device_file_name: String,
    pub device: Option<DeviceInfo>,

    // 是否需要提供shared_stack的使用方式，开放给别的进程使用？ 默认为true
    // 如果要以ShareObjectStack模式使用，那么必须配置为true
    pub shared_stack: bool,

    // 是否在同进程提供shared_object_stack的使用方式，默认为false
    // 需要在shared_stack=true的情况下，shared_stack_stub才可以设置为true
    pub shared_stack_stub: bool,

    // 协议栈是否开启sync服务，client device和ood device必须要配套同时开启或者同时关闭
    // 默认开启
    pub sync_service: bool,

    // 是不是移动端
    pub is_mobile_stack: bool,
}

impl Default for CyfsServiceLoaderParam {
    fn default() -> Self {
        Self {
            id: None,
            isolate: None,
            non_http_addr: "127.0.0.1:0".to_owned(),
            non_ws_addr: Some("127.0.0.1:0".to_owned()),
            bdt_port: 10001,
            bdt_endpoints: None,

            device_file_name: "device".to_owned(),
            device: None,
            shared_stack: true,
            shared_stack_stub: false,
            sync_service: true,
            is_mobile_stack: false,
        }
    }
}
pub struct CyfsServiceLoaderConfig {
    // 支持单table(单个stack)和列表模式(多个stack)
    pub node: toml::Value,
}

impl CyfsServiceLoaderConfig {
    pub fn new(param: CyfsServiceLoaderParam) -> BuckyResult<Self> {
        // 生成默认配置
        let config = Self::gen_default_config(&param);

        info!("default config: {}", config);

        // 如果外部显式的传入了device，那么动态添加到device_manager
        if let Some(device_info) = param.device {
            LOCAL_DEVICE_MANAGER.add(&param.device_file_name, device_info)?;
        }

        let node = Self::load_string_config(&config)?;

        Ok(Self { node })
    }

    pub fn new_from_config(node: toml::Value) -> BuckyResult<Self> {
        info!("will use config: {:?}", toml::to_string(&node));

        Ok(Self { node })
    }

    pub fn new_from_string(config: &str) -> BuckyResult<Self> {
        info!("will use config: {}", config);

        let node = Self::load_string_config(&config)?;
        Ok(Self { node })
    }

    pub fn gen_bdt_endpoints(param: &BdtEndPointParams) -> String {
        let mut ret = BDT_ENDPOINTS
            .replace("${bdt_port}", &param.bdt_port.to_string())
            .replace("${system_default}", &param.is_mobile_stack.to_string());

        // $xxx是var变量，被替换成具体值后，不再依赖varmanager的全局变量
        if let Some(ip_v4) = &param.none_local_ip_v4 {
            ret = ret.replace("$none_local_ip_v4", &ip_v4);
        }

        if let Some(ip_v6) = &param.ip_v6 {
            ret = ret.replace("$ip_v6", &ip_v6);
        }

        ret
    }

    fn gen_default_config(param: &CyfsServiceLoaderParam) -> String {
        // 必须先替换bdt_endpoints
        let bdt_endpoints = match &param.bdt_endpoints {
            Some(v) => v.as_str(),
            None => BDT_ENDPOINTS,
        };

        let bdt_config = BDT_CONFIG
            .replace("${device_file_name}", &param.device_file_name)
            .replace("${endpoints}", bdt_endpoints);

        let ret = NON_CONFIG
            .replace("${http_listener}", &param.non_http_addr)
            .replace("${bdt_config}", &bdt_config)
            .replace("${bdt_port}", &param.bdt_port.to_string())
            .replace("${system_default}", &param.is_mobile_stack.to_string())
            .replace("${id}", &param.id.as_ref().unwrap_or(&"default".to_owned()))
            .replace("${shared_stack}", &param.shared_stack.to_string())
            .replace("${shared_stack_stub}", &param.shared_stack_stub.to_string())
            .replace("${sync_service}", &param.sync_service.to_string())
            .replace(
                "${isolate}",
                &param.isolate.as_ref().unwrap_or(&"".to_owned()),
            )
            .replace(
                "${http-bdt-vport}",
                &cyfs_base::NON_STACK_BDT_VPORT.to_string(),
            );

        // 根据是否传入了ws_addr，选择enable=true或者enable=false
        if let Some(ws_addr) = &param.non_ws_addr {
            ret.replace("${ws_listener}", &ws_addr)
                .replace("${ws_enable}", "true")
        } else {
            ret.replace("${ws_enable}", "false")
        }
    }

    // 直接加载字符串形式的config, 并提取里面的stack根节点
    fn load_string_config(config: &str) -> BuckyResult<toml::Value> {
        let cfg_node: toml::Value = match toml::from_str(config) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!("load toml config error, value={}, err={}", config, e);
                error!("{}", msg);
                return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
            }
        };

        // 提取stack根节点配置
        let mut cfg_node = match cfg_node {
            toml::Value::Table(v) => v,
            _ => {
                let msg = format!("invalid toml root, value={}", config);
                error!("{}", msg);
                return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
            }
        };

        let stack = cfg_node.remove("stack");
        if stack.is_none() {
            let msg = format!("invalid toml root, stack node not found! value={}", config);
            error!("{}", msg);
            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
        }

        Ok(stack.unwrap())
    }

    // convert single node mode to vec mode
    // 支持一级的table和两级的数组两种模式
    pub fn root_to_list(cfg_node: toml::Value) -> BuckyResult<Vec<toml::value::Table>> {
        match cfg_node {
            toml::Value::Table(cfg) => Ok(vec![cfg]),
            toml::Value::Array(list) => {
                let mut result = vec![];
                for cfg_node in list {
                    match cfg_node {
                        toml::Value::Table(cfg) => {
                            result.push(cfg);
                        }
                        _ => {
                            let msg = format!(
                                "stack config list item invalid format! config={:?}",
                                cfg_node
                            );
                            error!("{}", msg);
                            return Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)));
                        }
                    }
                }

                Ok(result)
            }
            _ => {
                let msg = format!(
                    "stack config root node invalid format! config={:?}",
                    cfg_node
                );
                error!("{}", msg);
                Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)))
            }
        }
    }

    pub fn reset_bdt_device(&mut self, device_name: &str) -> BuckyResult<()> {
        match &mut self.node {
            toml::Value::Table(cfg) => Self::reset_stack_bdt_device(cfg, device_name),
            toml::Value::Array(list) => {
                // only support one stack in list!!
                if list.len() > 1 {
                    let msg = format!("stack config list is more than one! config={:?}", list);
                    error!("{}", msg);
                    return Err(BuckyError::from((BuckyErrorCode::UnSupport, msg)));
                }

                let cfg_node = list.last_mut().unwrap();
                match cfg_node {
                    toml::Value::Table(cfg) => Self::reset_stack_bdt_device(cfg, device_name),
                    _ => {
                        let msg = format!(
                            "stack config list item invalid format! config={:?}",
                            cfg_node
                        );
                        error!("{}", msg);
                        Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)))
                    }
                }
            }
            _ => {
                let msg = format!(
                    "stack config root node invalid format! config={:?}",
                    self.node
                );
                error!("{}", msg);
                Err(BuckyError::from((BuckyErrorCode::InvalidFormat, msg)))
            }
        }
    }

    // change the follow param:
    // [stack.bdt.config]
    // device = "device"
    fn reset_stack_bdt_device(node: &mut toml::value::Table, device_name: &str) -> BuckyResult<()> {
        match node.get_mut("bdt") {
            Some(v) => {
                if !v.is_table() {
                    let msg = format!("invalid non stack.bdt field format: {:?}", v);
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                let bdt_config = v.as_table_mut().unwrap();

                Self::reset_stack_bdt_config_device(bdt_config, device_name)?;
            }
            None => {
                let mut bdt_config = toml::value::Table::new();
                Self::reset_stack_bdt_config_device(&mut bdt_config, device_name)?;

                node.insert("bdt".to_owned(), toml::Value::Table(bdt_config));
            }
        }

        Ok(())
    }

    fn reset_stack_bdt_config_device(
        node: &mut toml::value::Table,
        device_name: &str,
    ) -> BuckyResult<()> {
        match node.get_mut("config") {
            Some(v) => {
                if !v.is_table() {
                    let msg = format!("invalid non stack.bdt.config field format: {:?}", v);
                    error!("{}", msg);

                    return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                }

                let bdt_config = v.as_table_mut().unwrap();

                bdt_config.insert(
                    "device".to_owned(),
                    toml::Value::String(device_name.to_owned()),
                );
            }
            None => {
                let mut bdt_config = toml::value::Table::new();
                bdt_config.insert(
                    "device".to_owned(),
                    toml::Value::String(device_name.to_owned()),
                );

                node.insert("config".to_owned(), toml::Value::Table(bdt_config));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod Test {
    use super::*;

    #[test]
    fn test_reset_bdt() {
        let list = CyfsServiceLoaderParam::default();
        let mut this = CyfsServiceLoaderConfig::new(list).unwrap();

        this.node
            .get_mut("bdt")
            .unwrap()
            .as_table_mut()
            .unwrap()
            .remove("config")
            .unwrap();
        println!("{:?}", toml::to_string(&this.node));

        this.reset_bdt_device("asdasd").unwrap();
        println!("{:?}", toml::to_string(&this.node));

        this.node.as_table_mut().unwrap().remove("bdt").unwrap();
        println!("{:?}", toml::to_string(&this.node));

        this.reset_bdt_device("asdasd").unwrap();
        println!("{:?}", toml::to_string(&this.node));
    }
}

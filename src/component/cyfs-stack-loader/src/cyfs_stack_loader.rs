use crate::bdt_loader::*;
use crate::ListenerUtil;
use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult};
use cyfs_stack::CyfsStackParams;
use cyfs_util::TomlHelper;
use cyfs_noc::NamedObjectStorageType;

use std::net::SocketAddr;

// 配置的默认协议栈的缺省名字
const DEFAULT_BDT_STACK_ID: &str = "default";

pub(crate) struct CyfsStackLoaderParams {
    // ObjectStack直接依赖的参数
    pub cyfs_stack_params: CyfsStackParams,

    // non-stack加载时候依赖的一些外部参数
    pub id: String,

    pub shared_stack_stub: bool,
}

impl Default for CyfsStackLoaderParams {
    fn default() -> Self {
        Self {
            cyfs_stack_params: CyfsStackParams::new_default(),
            id: DEFAULT_BDT_STACK_ID.to_owned(),
            shared_stack_stub: false,
        }
    }
}

impl CyfsStackLoaderParams {
    pub fn id(&self) -> &str {
        self.id.as_str()
    }

    pub fn is_default(&self) -> bool {
        self.id == DEFAULT_BDT_STACK_ID
    }
}

// 用以加载non-stack+bdt-stack的toml格式的参数
pub(crate) struct CyfsStackConfigLoader {
    params: CyfsStackLoaderParams,
    bdt_loader: BdtConfigLoader,
}

impl Into<(CyfsStackLoaderParams, BdtParams)> for CyfsStackConfigLoader {
    fn into(self) -> (CyfsStackLoaderParams, BdtParams) {
        (self.params, self.bdt_loader.into())
    }
}

impl CyfsStackConfigLoader {
    pub fn new(params: CyfsStackLoaderParams, bdt_params: BdtParams) -> Self {
        Self {
            params,
            bdt_loader: BdtConfigLoader::new(bdt_params),
        }
    }

    pub fn load(&mut self, node: &toml::value::Table) -> BuckyResult<()> {
        for (k, v) in node {
            match k.as_str() {
                "config" => {
                    if !v.is_table() {
                        let msg = format!("invalid non stack.config field format: {:?}", v);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }

                    self.load_config(v.as_table().unwrap())?;
                }

                "front" => {
                    if !v.is_table() {
                        let msg = format!("invalid non stack.front field format: {:?}", v);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }

                    self.load_front(v.as_table().unwrap())?;
                }

                "noc" => {
                    if !v.is_table() {
                        let msg = format!("invalid non stack.noc field format: {:?}", v);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }

                    self.load_noc(v.as_table().unwrap())?;
                }

                "interface" => {
                    self.load_interfaces(v)?;
                }

                "meta" => {
                    if !v.is_table() {
                        let msg = format!("invalid non stack.meta field format: {:?}", v);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }

                    self.load_meta(v.as_table().unwrap())?;
                }

                "bdt" => {
                    if !v.is_table() {
                        let msg = format!("invalid non stack.bdt field format: {:?}", v);
                        error!("{}", msg);

                        return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
                    }

                    self.bdt_loader.load(v.as_table().unwrap())?;
                }

                _ => {
                    warn!("unknown non stack field: {}", k.as_str());
                }
            }
        }

        Ok(())
    }

    fn load_config(&mut self, node: &toml::value::Table) -> BuckyResult<()> {
        for (k, v) in node {
            match k.as_str() {
                "id" => {
                    self.params.id = TomlHelper::decode_from_string(v)?;
                }

                "shared_stack" => {
                    self.params.cyfs_stack_params.config.shared_stack =
                        TomlHelper::decode_from_boolean(v)?;
                }
                "shared_stack_stub" => {
                    self.params.shared_stack_stub = TomlHelper::decode_from_boolean(v)?;
                }

                "sync_service" => {
                    self.params.cyfs_stack_params.config.sync_service =
                        TomlHelper::decode_from_boolean(v)?;
                }

                "isolate" => {
                    if !v.is_str() {
                        error!("invalid object stack.isolate field format: {:?}", v);
                        return Err(BuckyError::from(BuckyErrorCode::InvalidFormat));
                    }

                    self.params.cyfs_stack_params.config.isolate = Some(v.as_str().unwrap().to_owned());
                }

                _ => {
                    warn!("unknown non stack.config field: {}", k.as_str());
                }
            }
        }

        Ok(())
    }

    fn load_front(&mut self, node: &toml::value::Table) -> BuckyResult<()> {
        for (k, v) in node {
            match k.as_str() {
                "enable" => {
                    self.params.cyfs_stack_params.front.enable = TomlHelper::decode_from_boolean(v)?;
                }

                _ => {
                    warn!("unknown non stack.front field: {}", k.as_str());
                }
            }
        }

        Ok(())
    }

    fn load_meta(&mut self, node: &toml::value::Table) -> BuckyResult<()> {
        for (k, v) in node {
            match k.as_str() {
                "target" => {
                    self.params.cyfs_stack_params.meta.target = TomlHelper::decode_from_string(v)?;
                }

                _ => {
                    warn!("unknown non stack.meta field: {}", k.as_str());
                }
            }
        }

        Ok(())
    }

    fn load_noc(&mut self, node: &toml::value::Table) -> BuckyResult<()> {
        let mut noc_type = None;

        for (k, v) in node {
            match k.as_str() {
                "type" => {
                    if !v.is_str() {
                        error!("invalid object stack noc type field format: {:?}", v);
                        return Err(BuckyError::from(BuckyErrorCode::InvalidFormat));
                    }

                    let v = v.as_str().unwrap();

         
                    if v == "mongodb" || v == "mongo" {
                        noc_type = Some(NamedObjectStorageType::MongoDB);
                        continue;
                    } else if v == "sqlite" {
                        noc_type = Some(NamedObjectStorageType::Sqlite);
                        continue;
                    }

                    error!("unsupport noc type: {}", v);
                    return Err(BuckyError::from(BuckyErrorCode::UnSupport));
                }

                _ => {
                    warn!("unknown object stack noc field: {}", k.as_str());
                }
            }
        }

        // 如果提供了配置，那么才需要覆盖默认的配置
        if let Some(noc_type) = noc_type {
            self.params.cyfs_stack_params.noc.noc_type = noc_type;
        }

        Ok(())
    }

    fn load_interfaces(&mut self, node: &toml::Value) -> BuckyResult<()> {
        if !node.is_array() {
            error!("invalid non stack.interface node format: {:?}", node);
            return Err(BuckyError::from(BuckyErrorCode::InvalidFormat));
        }

        // 只要配置了listener字段，那么就需要清除掉所有的listener的默认配置
        self.params.cyfs_stack_params.interface.bdt_listeners.clear();
        self.params.cyfs_stack_params.interface.tcp_listeners.clear();
        self.params.cyfs_stack_params.interface.ws_listener = None;

        let node = node.as_array().unwrap();
        for listener_node in node {
            if !listener_node.is_table() {
                error!(
                    "invalid non stack.interface node format: {:?}",
                    listener_node
                );
                continue;
            }

            let listener_node = listener_node.as_table().unwrap();
            let type_node = listener_node.get("type");
            if type_node.is_none() {
                error!(
                    "non stack.interface type field missing: {:?}",
                    listener_node
                );
                continue;
            }

            let type_node = type_node.unwrap();
            match type_node.as_str().unwrap_or("") {
                "http" => self.load_tcp_interface(listener_node)?,

                "http-bdt" => self.load_bdt_interface(listener_node)?,

                "ws" => self.load_ws_interface(listener_node)?,

                "datagram_bdt" => {
                    unimplemented!();
                }

                v @ _ => {
                    error!("unknown non stack.interface type: {}", v);
                    continue;
                }
            };
        }

        Ok(())
    }

    fn load_tcp_interface(&mut self, node: &toml::value::Table) -> BuckyResult<()> {
        let listener_list: Vec<SocketAddr> = ListenerUtil::load_native_listener("http", node)?;
        assert!(listener_list.len() > 0);

        for addr in listener_list {
            if self
                .params
                .cyfs_stack_params
                .interface
                .tcp_listeners
                .iter()
                .find(|&x| *x == addr)
                .is_none()
            {
                info!("new non stack http interface addr: {}", addr);
                self.params.cyfs_stack_params.interface.tcp_listeners.push(addr);
            } else {
                error!("conflict non stack http interface addr: {}", addr);
            }
        }

        Ok(())
    }

    fn load_bdt_interface(&mut self, server_node: &toml::value::Table) -> BuckyResult<()> {
        let mut vport: Option<u16> = None;

        for (k, v) in server_node {
            match k.as_str() {
                "vport" => match cyfs_util::parse_port_from_toml_value(v) {
                    Ok(port) => vport = Some(port),
                    Err(e) => {
                        return Err(e);
                    }
                },
                _ => {
                    // error!("unknown tcp server field: {}", k.as_str());
                }
            }
        }

        if vport.is_none() {
            return BuckyError::error_with_log(format!(
                "invalid non stack.iterface block, vport not specified! v={:?}",
                server_node
            ));
        }

        let vport = vport.unwrap();
        if self
            .params
            .cyfs_stack_params
            .interface
            .bdt_listeners
            .iter()
            .find(|&x| *x == vport)
            .is_none()
        {
            info!("new non stack.bdt.endpoint listener vport: {}", vport);
            self.params.cyfs_stack_params.interface.bdt_listeners.push(vport);
        } else {
            error!("conflict non stack.interface vport: {}", vport);
        }

        Ok(())
    }

    fn load_ws_interface(&mut self, node: &toml::value::Table) -> BuckyResult<()> {
        // 首先读取enable
        let enable = match node.get("enable") {
            Some(v) => v.as_bool().unwrap_or(false),
            None => false,
        };

        if !enable {
            warn!("non stack ws service disabled! node={:?}", node);
            return Ok(());
        }

        let mut listener_list: Vec<SocketAddr> = ListenerUtil::load_native_listener("ws", node)?;
        assert!(listener_list.len() > 0);

        // 目前只支持一个ws listener
        if listener_list.len() > 1 {
            warn!(
                "only one ws interface support! now will use first one, listeners={:?}",
                listener_list
            );
        }

        self.params.cyfs_stack_params.interface.ws_listener = Some(listener_list.pop().unwrap());

        Ok(())
    }
}

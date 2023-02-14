use super::random_port::*;
use crate::bdt_loader::*;
use crate::cyfs_stack_loader::*;
use crate::{KNOWN_OBJECTS_MANAGER, VAR_MANAGER};
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_stack::{CyfsStack, CyfsStackKnownObjects};
use cyfs_bdt_ext::BdtStackParams;
use cyfs_util::{LOCAL_DEVICE_MANAGER, DeviceInfo};

// Temporarily disable all ipv6 addresses!
const IS_DISABLE_IPV6: bool = true;

pub(crate) struct StackInfo {
    pub stack_params: CyfsStackLoaderParams,
    pub bdt_params: BdtParams,

    device_info: Option<DeviceInfo>,

    bdt_stack: Option<StackGuard>,
    cyfs_stack: Option<CyfsStack>,
}

impl StackInfo {
    pub fn new() -> Self {
        Self {
            device_info: None,
            bdt_stack: None,

            stack_params: CyfsStackLoaderParams::default(),
            bdt_params: BdtParams::default(),

            cyfs_stack: None,
        }
    }

    pub fn id(&self) -> &str {
        self.stack_params.id()
    }

    pub fn is_default(&self) -> bool {
        self.stack_params.is_default()
    }

    pub fn device_id(&self) -> DeviceId {
        self.device_info.as_ref().unwrap().device.desc().device_id()
    }

    pub fn local_addr(&self) -> String {
        assert!(self.bdt_params.endpoint.len() > 0);
        format!("{}", self.bdt_params.endpoint[0].addr())
    }

    pub fn bdt_stack(&self) -> Option<&StackGuard> {
        self.bdt_stack.as_ref()
    }

    pub fn cyfs_stack(&self) -> Option<&CyfsStack> {
        self.cyfs_stack.as_ref()
    }

    pub fn load(mut self, node: &toml::value::Table) -> BuckyResult<Self> {
        let mut loader = CyfsStackConfigLoader::new(self.stack_params, self.bdt_params);
        loader.load(node)?;

        let (stack_params, bdt_params) = loader.into();
        self.stack_params = stack_params;
        self.bdt_params = bdt_params;

        // 加载device_info
        self.load_device_info()?;

        Ok(self)
    }

    pub async fn init(&mut self) -> BuckyResult<()> {
        // 添加到全局变量管理器
        {
            let key = if self.is_default() {
                "device_id".to_owned()
            } else {
                format!("{}_device_id", self.bdt_params.device)
            };

            VAR_MANAGER.add(key, self.device_id().to_string());
        }

        // 准备bdt协议栈的初始化必要参数
        let btd_stack_params = self.init_bdt_stack_params();

        let known_objects = CyfsStackKnownObjects {
            list: KNOWN_OBJECTS_MANAGER.clone_objects(),
            mode: KNOWN_OBJECTS_MANAGER.get_mode(),
        };

        // 初始化object_stack
        let cyfs_stack = CyfsStack::open(
            btd_stack_params,
            self.stack_params.cyfs_stack_params.clone(),
            known_objects,
        )
        .await?;

        assert!(self.bdt_stack.is_none());
        self.bdt_stack = Some(cyfs_stack.bdt_stack().clone());

        assert!(self.cyfs_stack.is_none());
        self.cyfs_stack = Some(cyfs_stack);

        Ok(())
    }

    fn init_bdt_stack_params(&mut self) -> BdtStackParams {
        assert!(!self.bdt_params.endpoint.is_empty());
        assert!(self.device_info.is_some());

        let device_info = self.device_info.as_mut().unwrap();

        let mut init_sn_peers = vec![];
        let mut init_pn_peers = vec![];

        // should not change the device's inner sn_list and pn_list
        info!("current device: {}", device_info.device.format_json());

        // only use the sn in local config dir
        for (id, sn) in cyfs_util::get_local_sn_desc() {
            info!("will use sn: {}", id);
            init_sn_peers.push(sn.to_owned());
        }

        for (id, pn) in cyfs_util::get_local_pn_desc() {
            info!("will use pn: {}", id);
            init_pn_peers.push(pn.to_owned());
        }

        let init_known_peers = cyfs_util::get_default_known_peers();

        // 初始化tcp端口映射
        let mut tcp_port_mapping = Vec::new();
        if let Some(port) = &self.bdt_params.tcp_port_mapping {
            for ep in &self.bdt_params.endpoint {
                if ep.protocol() == cyfs_base::Protocol::Tcp {
                    tcp_port_mapping.push((ep.clone(), *port));
                }
            }
        }

        let bdt_param = BdtStackParams {
            device: device_info.device.clone(),
            tcp_port_mapping,
            secret: device_info.private_key.as_ref().unwrap().clone(),
            known_sn: init_sn_peers,
            known_passive_pn: init_pn_peers,
            known_device: init_known_peers,
            udp_sn_only: self.bdt_params.udp_sn_only,
        };

        bdt_param
    }

    fn load_device_info(&mut self) -> BuckyResult<()> {
        assert!(!self.bdt_params.endpoint.is_empty());
        assert!(self.device_info.is_none());

        let ret = LOCAL_DEVICE_MANAGER.load(&self.bdt_params.device);
        if let Err(e) = ret {
            return Err(e);
        }

        let mut device_info = ret.unwrap();

        // 初始化bdt协议栈必须有私钥
        if device_info.private_key.is_none() {
            let msg = format!("{}.sec not found or invalid!", self.bdt_params.device);
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        // try generate random bdt port is configed with zero port
        let device_id = device_info.device.desc().calculate_id().to_string();
        RandomPortGenerator::prepare_endpoints(&device_id, &mut self.bdt_params.endpoint)?;

        let mut endpoints = self.bdt_params.endpoint.clone();
        if IS_DISABLE_IPV6 {
            endpoints.retain(|ep| {
                if ep.addr().is_ipv6() {
                    warn!("ipv6 addr will been disabled! ep={}", ep);
                    false
                } else {
                    true
                }
            });
        }

        device_info
            .device
            .mut_connect_info()
            .mut_endpoints()
            .append(&mut endpoints);

        self.device_info = Some(device_info);

        Ok(())
    }
}

use crate::bdt_loader::*;
use crate::cyfs_stack_loader::*;
use crate::{DeviceInfo, LOCAL_DEVICE_MANAGER};
use crate::{KNOWN_OBJECTS_MANAGER, VAR_MANAGER};
use cyfs_base::*;
use cyfs_bdt::StackGuard;
use cyfs_lib::SharedCyfsStack;
use cyfs_stack::{BdtStackParams, CyfsStack};

pub(crate) struct StackInfo {
    pub stack_params: CyfsStackLoaderParams,
    pub bdt_params: BdtParams,

    device_info: Option<DeviceInfo>,

    bdt_stack: Option<StackGuard>,
    cyfs_stack: Option<CyfsStack>,

    shared_cyfs_stack: Option<SharedCyfsStack>,
}

impl StackInfo {
    pub fn new() -> Self {
        Self {
            device_info: None,
            bdt_stack: None,

            stack_params: CyfsStackLoaderParams::default(),
            bdt_params: BdtParams::default(),

            cyfs_stack: None,

            shared_cyfs_stack: None,
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

    pub fn shared_cyfs_stack(&self) -> Option<&SharedCyfsStack> {
        self.shared_cyfs_stack.as_ref()
    }

    async fn init_shared_cyfs_stack(&mut self) -> BuckyResult<()> {
        assert!(self.shared_cyfs_stack.is_none());
        assert!(self.cyfs_stack.is_some());

        let interface = self.cyfs_stack.as_ref().unwrap().interface();
        if interface.is_none() {
            let msg = format!(
                "shared stack not enabled! should use config: shared_stack=true, id={}",
                self.id()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
        }

        // non-object的本地http地址
        // FIXME 以后可以选择是否开启本地http地址
        // FIXME 以后同进程直接使用native的ObjectStack
        let shared_cyfs_stack = self
            .cyfs_stack
            .as_ref()
            .unwrap()
            .open_shared_object_stack(None)
            .await
            .map_err(|e| {
                error!(
                    "shared object stack init failed! id={}. err={}",
                    self.id(),
                    e
                );
                e
            })?;

        self.shared_cyfs_stack = Some(shared_cyfs_stack);

        info!("init shared object stack success! id={}", self.id());

        Ok(())
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

        // 初始化object_stack
        let cyfs_stack = CyfsStack::open(
            btd_stack_params,
            self.stack_params.cyfs_stack_params.clone(),
            KNOWN_OBJECTS_MANAGER.clone_objects(),
        )
        .await?;

        assert!(self.bdt_stack.is_none());
        self.bdt_stack = Some(cyfs_stack.bdt_stack().clone());

        assert!(self.cyfs_stack.is_none());
        self.cyfs_stack = Some(cyfs_stack);

        if self.stack_params.shared_stack_stub {
            self.init_shared_cyfs_stack().await?;
        }

        Ok(())
    }

    fn init_bdt_stack_params(&mut self) -> BdtStackParams {
        assert!(!self.bdt_params.endpoint.is_empty());
        assert!(self.device_info.is_some());

        let device_info = self.device_info.as_mut().unwrap();

        let mut init_sn_peers = vec![];
        let mut init_pn_peers = vec![];
        let sn = cyfs_util::get_default_sn_desc();

        let mut device_changed = false;
        // 如果device没有指定sn，那么使用当前sn
        let device_sn_list = device_info.device.mut_connect_info().mut_sn_list();
        if device_sn_list.is_empty() {
            let sn_id = sn.desc().device_id();
            info!(
                "device.desc sn list is empty! now will use default sn: {}",
                sn_id
            );
            device_sn_list.push(sn_id);
            device_changed = true;
        } else {
            info!("device.desc had inner sn list: {:?}", device_sn_list);
        }
        init_sn_peers.push(sn);

        if let Some(pn) = cyfs_util::get_pn_desc() {
            let device_pn_list = device_info.device.mut_connect_info().mut_passive_pn_list();

            // 如果device没有指定pn，那么使用当前pn
            if device_pn_list.is_empty() {
                let pn_id = pn.desc().device_id();
                info!(
                    "device.desc pn list is empty! now will use default pn: {}",
                    pn_id
                );
                device_pn_list.push(pn_id);
                device_changed = true;
            } else {
                info!("device.desc had inner pn list: {:?}", device_pn_list);
            }

            init_pn_peers.push(pn);
        }

        // device更新sn或者pn后，必须更新update_time
        if device_changed {
            device_info
                .device
                .body_mut()
                .as_mut()
                .unwrap()
                .increase_update_time(bucky_time_now());
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

        device_info
            .device
            .mut_connect_info()
            .mut_endpoints()
            .append(&mut self.bdt_params.endpoint.clone());

        self.device_info = Some(device_info);

        Ok(())
    }
}

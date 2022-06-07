use super::proxy::CyfsProxy;
use super::stack::{CyfsStackIns, CyfsStackInsConfig, UpdateStackNetworkParams};
use cyfs_base::*;
use ood_control::*;

use std::path::{Path, PathBuf};

pub(crate) struct CyfsRuntime {
    config_file: PathBuf,
    stack: CyfsStackIns,
    proxy: CyfsProxy,
}

impl CyfsRuntime {
    pub fn new(stack_config: CyfsStackInsConfig) -> Self {
        let config_file = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("runtime")
            .join("runtime.toml");

        let proxy = CyfsProxy::new(&stack_config);
        Self {
            config_file,
            stack: CyfsStackIns::new(stack_config, proxy.clone()),
            proxy,
        }
    }

    // 网络ip发生变化后，动态的通知协议栈切换网络ip
    pub async fn update_network(&self, param: UpdateStackNetworkParams) {
        self.stack.update_network(param).await
    }

    pub async fn start(&mut self) -> BuckyResult<()> {
        // 首先加载stack
        self.load_config().await?;

        self.proxy.start().await?;

        self.stack.start().await?;

        // start control interface
        let param = ControlInterfaceParam {
            mode: OODControlMode::Runtime,
            tcp_port: None,

            // if device binded already，should not bind public address to avoid risk
            require_access_token: !OOD_CONTROLLER.is_bind(),
            tcp_host: None,
            addr_type: ControlInterfaceAddrType::V4,
        };

        let control_interface = ControlInterface::new(param, &OOD_CONTROLLER);
        if let Err(e) = control_interface.start().await {
            return Err(e);
        }

        Ok(())
    }

    pub async fn load_config(&mut self) -> BuckyResult<()> {
        let config_file = self.config_file.as_path();
        if !config_file.exists() {
            let msg = format!(
                "load runtime config file but not exists! file={}",
                config_file.display()
            );
            warn!("{}", msg);
            return Ok(());
        }

        let ret = self.load_as_toml(&config_file).await?;

        self.parse_config(ret).await
    }

    async fn load_as_toml(&self, file_path: &Path) -> BuckyResult<toml::Value> {
        let content = async_std::fs::read_to_string(file_path)
            .await
            .map_err(|e| {
                let msg = format!(
                    "load config file as string error! file={}, err={}",
                    file_path.display(),
                    e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let u: toml::Value = toml::from_str(&content).map_err(|e| {
            let msg = format!(
                "parse config as toml error! file={}, content={}, err={}",
                file_path.display(),
                content,
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        info!("load runtime config success! file={}", file_path.display());

        Ok(u)
    }

    async fn parse_config(&mut self, cfg_node: toml::Value) -> BuckyResult<()> {
        if !cfg_node.is_table() {
            let msg = format!(
                "config root node invalid format! file={}",
                self.config_file.display()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let mut cfg_node: toml::value::Table = match cfg_node {
            toml::Value::Table(v) => v,
            _ => unreachable!(),
        };

        // 优先加载non协议栈
        if let Some(v) = cfg_node.remove("stack") {
            self.stack.set_config(v).await?;
        }

        // TODO 遍历加载其余节点
        Ok(())
    }
}

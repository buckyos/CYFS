use base::CyfsServiceLoaderConfig;
use std::path::{Path, PathBuf};

use crate::control::HttpControlInterface;
use crate::server::http::HttpServerManager;
use crate::server::stream::StreamServerManager;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_stack_loader::STACK_MANAGER;
use cyfs_util::EventListenerAsyncRoutine;

struct ZoneRoleChangedNotify {}

#[async_trait::async_trait]
impl
    EventListenerAsyncRoutine<
        RouterEventZoneRoleChangedEventRequest,
        RouterEventZoneRoleChangedEventResult,
    > for ZoneRoleChangedNotify
{
    async fn call(
        &self,
        param: &RouterEventZoneRoleChangedEventRequest,
    ) -> BuckyResult<RouterEventZoneRoleChangedEventResult> {
        warn!(
            "gateway recv zone role changed notify! now will restart! {}",
            param
        );
        async_std::task::spawn(async {
            async_std::task::sleep(std::time::Duration::from_secs(3)).await;
            std::process::exit(0);
        });

        let resp = RouterEventResponse {
            call_next: true,
            handled: true,
            response: None,
        };

        Ok(resp)
    }
}

/*
impl EventListenerSyncRoutine<ZoneRoleChangedParam, ()> for ZoneRoleChangedNotify {
    fn call(&self, param: &ZoneRoleChangedParam) -> BuckyResult<()> {
        warn!(
            "gateway recv zone role changed notify! now will restart! {:?}",
            param
        );
        async_std::task::spawn(async {
            async_std::task::sleep(std::time::Duration::from_secs(3)).await;
            std::process::exit(0);
        });

        Ok(())
    }
}
*/


pub(crate) struct Gateway {
    config_file: PathBuf,

    stream_server_manager: StreamServerManager,
    http_server_manager: HttpServerManager,
    http_control_interface: HttpControlInterface,
}

impl Gateway {
    pub fn new() -> Self {
        let config_file = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("gateway")
            .join("gateway.toml");

        let stream_server_manager = StreamServerManager::new();
        let http_server_manager = HttpServerManager::new();
        let http_control_interface =
            HttpControlInterface::new(stream_server_manager.clone(), http_server_manager.clone());

        Self {
            config_file,
            stream_server_manager,
            http_server_manager,
            http_control_interface,
        }
    }

    pub async fn load_config(&self) -> BuckyResult<()> {
        let config_file = self.config_file.as_path();
        if !config_file.exists() {
            let msg = format!(
                "load system config file not found! file={}",
                config_file.display()
            );
            error!("{}", msg);
            return Err(BuckyError::from(msg));
        }

        let ret = self.load_as_toml(&config_file).await?;

        self.parse_config(ret).await
    }

    async fn load_as_toml(&self, file_path: &Path) -> BuckyResult<toml::Value> {
        let value = async_std::fs::read_to_string(file_path)
            .await
            .map_err(|e| {
                let msg = format!(
                    "load gateway.toml error! file={}, err={}",
                    file_path.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        let u: toml::Value = toml::from_str(&value).map_err(|e| {
            let msg = format!(
                "parse gateway.toml error! file={}, value={}, err={}",
                file_path.display(),
                value,
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        Ok(u)
    }

    async fn parse_config(&self, cfg_node: toml::Value) -> BuckyResult<()> {
        if !cfg_node.is_table() {
            let msg = format!(
                "config root node invalid format! file={}",
                self.config_file.display()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        let mut cfg_node = match cfg_node {
            toml::Value::Table(v) => v,
            _ => unreachable!(),
        };

        // 优先加载non协议栈
        if let Some(v) = cfg_node.remove("stack") {
            let config = CyfsServiceLoaderConfig::new_from_config(v)?;

            STACK_MANAGER.load(config.into()).await?;
        }

        // 遍历加载其余节点
        for (k, v) in cfg_node {
            match k.as_str() {
                "config" => {}
                "stream" => {
                    if v.is_array() {
                        self.stream_server_manager.load(v.as_array().unwrap())?;
                    } else {
                        let msg = format!("config invalid stream node format: {:?}", v);
                        error!("{}", msg);

                        return Err(BuckyError::from(msg));
                    }
                }
                "http" => {
                    if v.is_array() {
                        self.http_server_manager.load(v.as_array().unwrap())?;
                    } else {
                        let msg = format!("config invalid http node format: {:?}", v);
                        error!("{}", msg);

                        return Err(BuckyError::from(msg));
                    }
                }
                _ => {
                    warn!("unknown service config node: {}", &k);
                }
            }
        }

        Ok(())
    }

    async fn init_stack() {
        let stack = STACK_MANAGER.get_default_cyfs_stack().unwrap();

        let notifier = ZoneRoleChangedNotify {};
        if let Err(e) = stack
            .open_uni_stack(&Some(cyfs_core::get_system_dec_app().to_owned()))
            .await
            .router_events()
            .zone_role_changed_event()
            .add_event("gateway-watcher", -1, Box::new(notifier))
            .await
        {
            error!("watch zone role changed event failed! {}", e);
        }
    }

    pub fn start(&self) {
        async_std::task::spawn(async move {
            Self::init_stack().await;
        });

        self.stream_server_manager.start();

        self.http_server_manager.start();

        self.http_control_interface.init();
    }

    pub async fn run(&self) {
        let _ = self.http_control_interface.run().await;
    }
}
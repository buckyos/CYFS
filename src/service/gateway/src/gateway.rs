use lazy_static::lazy_static;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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
pub struct Gateway {
    config_file: PathBuf,
    pub stream_server_manager: StreamServerManager,
    pub http_server_manager: HttpServerManager,
}

impl Gateway {
    pub fn new() -> Self {
        let config_file = ::cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("gateway")
            .join("gateway.toml");

        Self {
            config_file,
            stream_server_manager: StreamServerManager::new(),
            http_server_manager: HttpServerManager::new(),
        }
    }

    pub async fn load_config(&mut self) -> Result<(), BuckyError> {
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

    async fn parse_config(&mut self, cfg_node: toml::Value) -> Result<(), BuckyError> {
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
            STACK_MANAGER.load(v).await?;
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
            .uni_stack()
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

        HttpControlInterface::init();
    }
}

lazy_static! {
    pub static ref GATEWAY: Mutex<Gateway> = Mutex::new(Gateway::new());
}

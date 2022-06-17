use super::anonymous::AnonymousManager;
use super::proxy::CyfsProxy;
use cyfs_base::*;
use cyfs_stack_loader::*;
use cyfs_util::*;
use ood_control::OOD_CONTROLLER;

use async_std::sync::{Arc, Mutex};

pub const PROXY_PORT: u16 = 38090;

#[derive(Debug, Clone)]
pub(crate) struct CyfsStackInsConfig {
    pub is_mobile_stack: bool,

    // if use anonymous identiy, default fasle
    pub anonymous: bool,

    // when anonymous=true, random_id=true will gen an new device id and save; random_id=false will try to use the saved one
    pub random_id: bool,

    // local Proxy service'r port, default is 38090
    pub proxy_port: u16,
}

#[derive(Debug)]
pub struct UpdateStackNetworkParams {
    pub ip_v4: String,
    pub ip_v6: Option<String>,
}

pub(crate) struct CyfsStackInsImpl {
    stack_config: CyfsStackInsConfig,

    config: Option<CyfsServiceLoaderConfig>,

    bdt_endpoints: std::sync::Mutex<BdtEndPointParams>,

    cyfs_stack: Option<CyfsStack>,

    proxy: CyfsProxy,
}

impl CyfsStackInsImpl {
    pub fn new(stack_config: CyfsStackInsConfig, proxy: CyfsProxy) -> Self {
        let params = BdtEndPointParams {
            none_local_ip_v4: None,
            ip_v6: if stack_config.is_mobile_stack {
                Some("::".to_owned())
            } else {
                None
            },
            bdt_port: cyfs_base::CYFS_RUNTIME_BDT_STACK_PORT,
            is_mobile_stack: stack_config.is_mobile_stack,
        };

        Self {
            stack_config,
            config: None,
            bdt_endpoints: std::sync::Mutex::new(params),
            cyfs_stack: None,
            proxy,
        }
    }

    pub fn set_config(&mut self, node: toml::Value) -> BuckyResult<()> {
        assert!(self.config.is_none());
        info!("will load non stack config: {:?}", toml::to_string(&node));
        let non_config = CyfsServiceLoaderConfig::new_from_config(node)?;
        self.config = Some(non_config);

        Ok(())
    }

    pub fn load_default_config(&mut self) -> BuckyResult<()> {
        let bdt_endpoints = self.gen_default_bdt_endpoints_config();

        let mut params = CyfsServiceLoaderParam::default();
        params.bdt_port = cyfs_base::CYFS_RUNTIME_BDT_STACK_PORT;
        params.bdt_endpoints = Some(bdt_endpoints);
        params.non_http_addr = format!("127.0.0.1:{}", cyfs_base::CYFS_RUNTIME_NON_STACK_HTTP_PORT);
        params.non_ws_addr = Some(format!(
            "127.0.0.1:{}",
            cyfs_base::CYFS_RUNTIME_NON_STACK_WS_PORT
        ));
        params.front_enable = true;

        let config = CyfsServiceLoaderConfig::new(params)?;
        info!(
            "will use default non stack config: {}",
            toml::to_string(&config.node).unwrap()
        );
        assert!(self.config.is_none());
        self.config = Some(config);

        Ok(())
    }

    fn gen_default_bdt_endpoints_config(&self) -> String {
        let params = self.bdt_endpoints.lock().unwrap();

        CyfsServiceLoaderConfig::gen_bdt_endpoints(&params)
    }

    async fn init_stack(&mut self) -> BuckyResult<()> {
        assert!(self.config.is_some());
        let config = self.config.take().unwrap();

        if let Err(e) = CyfsServiceLoader::load(config).await {
            error!("load non stack failed! err={}", e);
            return Err(e);
        }

        assert!(self.cyfs_stack.is_none());
        let stack = CyfsServiceLoader::default_object_stack();
        self.proxy.bind_non_stack(stack.clone());
        self.cyfs_stack = Some(stack);

        Ok(())
    }

    pub fn update_network(&self, param: UpdateStackNetworkParams) {
        log::info!("will update network: {:?}", param);

        assert!(param.ip_v4.len() > 0);

        // 更新保存的bdt_endpoints配置
        {
            let mut bdt_params = self.bdt_endpoints.lock().unwrap();
            bdt_params.none_local_ip_v4 = Some(param.ip_v4);
            if let Some(ip_v6) = param.ip_v6 {
                bdt_params.ip_v6 = Some(ip_v6);
            }
        }

        // 如果bdt协议栈已经加载，那么需要reset
        if let Some(stack) = &self.cyfs_stack {
            let config = self.gen_default_bdt_endpoints_config();
            let endpoints = CyfsServiceLoader::load_endpoints(&config).unwrap();
            // bdt这里要求reset用的endpoint都配置default字段位true
            /* 统一通过配置设置system_default字段，这里不需要单独配置了
            for endpoint in &mut endpoints {
                endpoint.set_system_default(true);
            }
             */
            log::info!("will reset bdt stack endpoints: {:?}", endpoints);
            // 这里不等待reset的结果，直接返回
            let cloned_stack = stack.clone();
            async_std::task::spawn(async move {
                if let Err(e) = cloned_stack.reset_network(&endpoints).await {
                    error!("reset bdt stack network failed! {}", e);
                }
            });
        }
    }
}

struct BindNotify {
    owner: CyfsStackIns,
}

impl EventListenerSyncRoutine<(), ()> for BindNotify {
    fn call(&self, _: &()) -> BuckyResult<()> {
        self.owner.on_bind();
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct CyfsStackIns(Arc<Mutex<CyfsStackInsImpl>>);

impl CyfsStackIns {
    pub fn new(stack_config: CyfsStackInsConfig, proxy: CyfsProxy) -> Self {
        Self(Arc::new(Mutex::new(CyfsStackInsImpl::new(
            stack_config,
            proxy,
        ))))
    }

    pub async fn set_config(&self, node: toml::Value) -> BuckyResult<()> {
        self.0.lock().await.set_config(node)
    }

    pub async fn update_network(&self, param: UpdateStackNetworkParams) {
        self.0.lock().await.update_network(param)
    }

    pub async fn start(&self) -> BuckyResult<()> {
        let mut stack = self.0.lock().await;
        if stack.config.is_none() {
            stack.load_default_config()?;
        }

        info!("stack_config={:?}", stack.stack_config);

        if stack.stack_config.anonymous {
            let am = AnonymousManager::new();
            let id = am.init(stack.stack_config.random_id);
            info!("runtime will run in anonymous mode with device_id={}", id);
            stack.config.as_mut().unwrap().reset_bdt_device(&id)?;

            stack.init_stack().await?;
        } else {
            if OOD_CONTROLLER.is_bind() {
                stack.init_stack().await?;
            } else {
                warn!("runtime device not bind yet!");

                let notify = BindNotify {
                    owner: self.clone(),
                };
                OOD_CONTROLLER.bind_event().on(Box::new(notify));
            }
        }

        Ok(())
    }

    fn on_bind(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            info!("device bind success, now will init non stack!");

            let mut stack = this.0.lock().await;

            // do nothing on anonymous mode(stack maybe running already!)
            if stack.stack_config.anonymous {
                warn!("bind success but stack is in anonymous mode!");
                return;
            }

            if let Err(e) = stack.init_stack().await {
                error!("init non stack failed! {}", e);
                let code: u32 = e.code().into();

                let _r = async_std::future::timeout(
                    std::time::Duration::from_secs(5),
                    async_std::future::pending::<()>(),
                )
                .await;

                std::process::exit(code as i32);
            } else {
                info!("init non stack success!");
            }
        });
    }
}

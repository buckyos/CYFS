use cyfs_base::BuckyResult;
use cyfs_lib::{SharedCyfsStack, BrowserSanboxMode};
use cyfs_stack_loader::{
    BdtEndPointParams, CyfsServiceLoader, CyfsServiceLoaderConfig, CyfsServiceLoaderParam,
    CyfsStack,
};

use std::sync::Mutex;

/*
const BDT_ENDPOINT_CONFIG: &str = r#"
[[stack.bdt.endpoint]]
optional = true
host = "${ip_v4}"
port = ${bdt_port}
protocol = "tcp"
system_default = true

[[stack.bdt.endpoint]]
optional = true
host = "${ip_v4}"
port = ${bdt_port}
protocol = "udp"
system_default = true

[[stack.bdt.endpoint]]
optional = true
host = "::"
port = ${bdt_port}
protocol = "tcp"
system_default = true

[[stack.bdt.endpoint]]
optional = true
host = "::"
port = ${bdt_port}
protocol = "udp"
system_default = true
"#;
*/

pub struct NonStub {
    cyfs_stack: Option<SharedCyfsStack>,
    local_object_stack: Option<CyfsStack>,

    device_file_name: String,
    bdt_endpoints: Mutex<Option<String>>,
    bdt_port: u16,
    non_http_addr: String,
    non_ws_addr: String,
}

impl NonStub {
    pub fn new(
        device_file_name: &str,
        bdt_port: u16,
        non_http_addr: &str,
        non_ws_addr: &str,
    ) -> Self {
        Self {
            cyfs_stack: None,
            local_object_stack: None,

            device_file_name: device_file_name.to_owned(),
            bdt_endpoints: Mutex::new(None),
            bdt_port,
            non_http_addr: non_http_addr.to_owned(),
            non_ws_addr: non_ws_addr.to_owned(),
        }
    }

    pub async fn update_network(&self, str: &str) {
        log::info!("will update network: {:?}", str);

        if str == "" {
            log::error!("will update network to empty addr! ignore it.");
            return;
        }

        // 生成对应的bdt_endpoints配置
        let bdt_endpoints_params = BdtEndPointParams {
            none_local_ip_v4: Some(str.to_owned()),
            ip_v6: Some("::".to_owned()),
            bdt_port: self.bdt_port,
            is_mobile_stack: true,
        };
        let ret = CyfsServiceLoaderConfig::gen_bdt_endpoints(&bdt_endpoints_params);
        *self.bdt_endpoints.lock().unwrap() = Some(ret);

        // 如果bdt协议栈已经加载，那么需要reset
        if let Some(stack) = &self.local_object_stack {
            let endpoints = CyfsServiceLoader::load_endpoints(
                self.bdt_endpoints.lock().unwrap().as_ref().unwrap(),
            )
            .unwrap();

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
                    log::error!("reset non stack network failed! {}", e);
                }
            });
        }
    }

    pub async fn init(&mut self) -> BuckyResult<()> {
        // 初始化前，必须已经调用过一次update_network了
        let bdt_endpoints = self.bdt_endpoints.lock().unwrap().clone();
        let param = CyfsServiceLoaderParam {
            id: None,
            isolate: None,
            non_http_addr: self.non_http_addr.clone(),
            non_ws_addr: Some(self.non_ws_addr.clone()),
            bdt_endpoints,
            bdt_port: self.bdt_port,
            device_file_name: self.device_file_name.clone(),
            device: None,
            shared_stack: true,
            shared_stack_stub: true,
            sync_service: true,
            is_mobile_stack: true,
            front_enable: true,
            browser_mode: BrowserSanboxMode::default(),
        };
        let config = CyfsServiceLoaderConfig::new(param)?;

        CyfsServiceLoader::load(config).await?;

        assert!(self.cyfs_stack.is_none());
        assert!(self.local_object_stack.is_none());

        self.local_object_stack = Some(CyfsServiceLoader::default_object_stack());
        let stack = CyfsServiceLoader::default_shared_object_stack();
        stack.online().await?;
        self.cyfs_stack = Some(stack);

        Ok(())
    }

    pub async fn restart_interface(&self) {
        if let Some(stack) = &self.local_object_stack {
            log::info!("will restart stack interface");
            // 这里不等待reset的结果，直接返回
            let cloned_stack = stack.clone();
            async_std::task::spawn(async move {
                if let Err(e) = cloned_stack.restart_interface().await {
                    log::error!("restart stack interface failed! {}", e);
                }
            });
        }
    }

    pub fn cyfs_stack(&self) -> SharedCyfsStack {
        self.cyfs_stack.as_ref().unwrap().clone()
    }
}

use once_cell::sync::OnceCell;
pub static NON_STUB: OnceCell<NonStub> = OnceCell::new();

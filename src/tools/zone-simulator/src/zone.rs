use crate::user::*;
use cyfs_base::*;
use cyfs_lib::*;
use cyfs_stack_loader::*;
use crate::loader::*;

use std::sync::Mutex;


#[derive(Debug, Clone)]
pub struct CyfsStackInsConfig {
    // the browser sanbox mode
    pub browser_mode: BrowserSanboxMode,
}

impl Default for CyfsStackInsConfig {
    fn default() -> Self {
        Self {
            browser_mode: BrowserSanboxMode::None,
        }
    }
}

struct SharedStackCacheItem {
    id: String,
    stack: SharedCyfsStack,
}

pub struct SharedStackCache {
    list: Mutex<Vec<SharedStackCacheItem>>,
}

impl SharedStackCache {
    fn new() -> Self {
        Self {
            list: Mutex::new(vec![]),
        }
    }

    pub fn instance() -> &'static SharedStackCache {
        use once_cell::sync::OnceCell;
        static SHARED_STACK_CACHE: OnceCell<SharedStackCache> = OnceCell::new();
        SHARED_STACK_CACHE.get_or_init(|| Self::new())
    }

    fn add(&self, id: String, stack: SharedCyfsStack) {
        let mut list = self.list.lock().unwrap();
        assert!(list.iter().find(|v| v.id == id).is_none());
        list.push(SharedStackCacheItem { id, stack });
    }

    pub fn get(&self, id: &str) -> Option<SharedCyfsStack> {
        self.list
            .lock()
            .unwrap()
            .iter()
            .find(|v| v.id == id)
            .map(|v| v.stack.clone())
    }
}

pub struct TestStack {
    device_info: DeviceInfo,
    stack_config: CyfsStackInsConfig,
    requestor_config: CyfsStackRequestorConfig,
}

impl TestStack {
    pub fn new(device_info: DeviceInfo, stack_config: CyfsStackInsConfig, requestor_config: CyfsStackRequestorConfig) -> Self {
        Self {
            device_info,
            stack_config,
            requestor_config,
        }
    }

    pub async fn init(self, ws: bool, bdt_port: u16, service_port: u16) {
        let device_id = self.device_info.device.desc().device_id();
        let device_id_str = device_id.to_string();

        let mut param = CyfsServiceLoaderParam::default();
        param.id = Some(device_id_str.clone());

        let isolate = format!("zone-simulator/{}", device_id_str);
        param.isolate = Some(isolate);

        param.non_http_addr = format!("127.0.0.1:{}", service_port);

        if !ws {
            param.non_ws_addr = None;
        } else {
            param.non_ws_addr = Some(format!("127.0.0.1:{}", service_port + 1));
        }

        param.bdt_port = bdt_port;

        param.device_file_name = device_id_str.clone();
        param.device = Some(self.device_info.clone());
        param.shared_stack = true;
        param.shared_stack_stub = true;
        param.front_enable = true;
        param.browser_mode = self.stack_config.browser_mode;

        let config = CyfsServiceLoaderConfig::new(param).unwrap();
        CyfsServiceLoader::direct_load(config).await.unwrap();

        // 等待协议栈初始化完毕
        async_std::task::sleep(std::time::Duration::from_secs(5)).await;

        // 初始化sharedobjectstack
        let stack = CyfsServiceLoader::cyfs_stack(Some(&device_id_str));
        let stack = stack.open_shared_object_stack(Some(DEC_ID.clone()), Some(self.requestor_config)).await.unwrap();

        stack
            .wait_online(Some(std::time::Duration::from_secs(10)))
            .await
            .unwrap();

        assert_eq!(stack.local_device_id().to_string(), device_id_str);

        SharedStackCache::instance().add(device_id_str, stack);
    }
}

pub struct TestZone {
    ws: bool,
    bdt_port: u16,
    service_port: u16,
    user: TestUser,
}

impl TestZone {
    pub fn new(ws: bool, bdt_port: u16, service_port: u16, user: TestUser) -> Self {
        Self {
            ws,
            bdt_port,
            service_port,
            user,
        }
    }

    fn random_requestor_config() -> CyfsStackRequestorConfig {
        fn random_select() -> CyfsStackRequestorType {
            if bucky_time_now() / 2 == 0 {
                CyfsStackRequestorType::Http
            } else {
                CyfsStackRequestorType::WebSocket
            }
        }

        CyfsStackRequestorConfig {
            non_service: random_select(),
            ndn_service: random_select(),
            util_service: random_select(),
            trans_service: random_select(),
            crypto_service: random_select(),
            root_state: random_select(),
            local_cache: random_select(),
        }
    }

    pub async fn init(&self, stack_config: &CyfsStackInsConfig) {
        let device_info = self.user.ood.clone();
        let bdt_port = self.bdt_port;
        let ws = self.ws;
        let service_port = self.service_port;
        let name = self.user.name().to_owned();
        let config = stack_config.to_owned();
        let handle1 = async_std::task::spawn(async move {
            let requestor_config = Self::random_requestor_config();
            let stack = TestStack::new(device_info, config, requestor_config);
            stack.init(ws, bdt_port, service_port).await;
            info!("init stack complete: user={}, stack=ood", name);
        });

        let device_info = self.user.device1.clone();
        let bdt_port = self.bdt_port + 1;
        let service_port = self.service_port + 2;
        let name = self.user.name().to_owned();
        let config = stack_config.to_owned();
        let handle2 = async_std::task::spawn(async move {
            let requestor_config = CyfsStackRequestorConfig::ws();
            let stack = TestStack::new(device_info, config, requestor_config);
            stack.init(ws, bdt_port, service_port).await;
            info!("init stack complete: user={}, stack=device1", name);
        });

        let device_info = self.user.device2.clone();
        let bdt_port = self.bdt_port + 2;
        let service_port = self.service_port + 4;
        let name = self.user.name().to_owned();
        let config = stack_config.to_owned();
        let handle3 = async_std::task::spawn(async move {
            let requestor_config = CyfsStackRequestorConfig::http();
            let stack = TestStack::new(device_info, config, requestor_config);
            stack.init(ws, bdt_port, service_port).await;
            info!("init stack complete: user={}, stack=device2", name);
        });

        let handle4 = if let Some(device_info) = self.user.standby_ood.clone() {
            let bdt_port = self.bdt_port + 3;
            let service_port = self.service_port + 6;
            let name = self.user.name().to_owned();
            let config = stack_config.to_owned();
            async_std::task::spawn(async move {
                let requestor_config = Self::random_requestor_config();
                let stack = TestStack::new(device_info, config, requestor_config);
                stack.init(ws, bdt_port, service_port).await;
                info!("init stack complete: user={}, stack=standby_ood", name);
            })
        } else {
            async_std::task::spawn(async move {})
        };

        futures::join!(handle1, handle2, handle3, handle4);

        info!("init zone complete! user={}", self.user.name());
    }
}

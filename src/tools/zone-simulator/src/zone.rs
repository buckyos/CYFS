use crate::user::*;
use cyfs_base::*;
use cyfs_stack_loader::*;

pub struct TestStack {
    device_info: DeviceInfo,
}

impl TestStack {
    pub fn new(device_info: DeviceInfo) -> Self {
        Self { device_info }
    }

    pub async fn init(&self, ws: bool, bdt_port: u16, service_port: u16) {
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

        let config = CyfsServiceLoaderConfig::new(param).unwrap();
        CyfsServiceLoader::direct_load(config).await.unwrap();

        // 等待协议栈初始化完毕
        async_std::task::sleep(std::time::Duration::from_secs(5)).await;

        // 初始化sharedobjectstack
        let stack = CyfsServiceLoader::shared_cyfs_stack(Some(&device_id_str));
        stack
            .wait_online(Some(std::time::Duration::from_secs(10)))
            .await
            .unwrap();

        assert_eq!(stack.local_device_id().to_string(), device_id_str);
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

    pub async fn init(&self) {
        let device_info = self.user.ood.clone();
        let bdt_port = self.bdt_port;
        let ws = self.ws;
        let service_port = self.service_port;
        let name = self.user.name().to_owned();
        let handle1 = async_std::task::spawn(async move {
            let stack = TestStack::new(device_info);
            stack.init(ws, bdt_port, service_port).await;
            info!("init stack complete: user={}, stack=ood", name);
        });

        let device_info = self.user.device1.clone();
        let bdt_port = self.bdt_port + 1;
        let service_port = self.service_port + 2;
        let name = self.user.name().to_owned();
        let handle2 = async_std::task::spawn(async move {
            let stack = TestStack::new(device_info);
            stack.init(ws, bdt_port, service_port).await;
            info!("init stack complete: user={}, stack=device1", name);
        });

        let device_info = self.user.device2.clone();
        let bdt_port = self.bdt_port + 2;
        let service_port = self.service_port + 4;
        let name = self.user.name().to_owned();
        let handle3 = async_std::task::spawn(async move {
            let stack = TestStack::new(device_info);
            stack.init(ws, bdt_port, service_port).await;
            info!("init stack complete: user={}, stack=device2", name);
        });

        let handle4 = if let Some(device_info) = self.user.standby_ood.clone() {
            let bdt_port = self.bdt_port + 3;
            let service_port = self.service_port + 6;
            let name = self.user.name().to_owned();
            async_std::task::spawn(async move {
                let stack = TestStack::new(device_info);
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

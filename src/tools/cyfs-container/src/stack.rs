use cyfs_base::*;
use cyfs_lib::*;
use cyfs_stack_loader::{CyfsServiceLoader, CyfsServiceLoaderConfig, CyfsStack};

use std::sync::{Arc, Mutex};

const STACK_CONFIG: &str = r#"
[{
    id: "${device}",
    endpoint: [{
        optional: true,
        host: "$none_local_ip_v4",
        port: ${bdt_port},
        protocol: "udp",
    },
    {
        optional: true,
        host: "$none_local_ip_v4",
        port: ${bdt_port},
        protocol: "tcp",
    },
    {
        optional: true,
        host: "$ip_v6",
        port: ${bdt_port},
        protocol: "udp",
    },
    {
        optional: true,
        host: "$ip_v6",
        port: ${bdt_port},
        protocol: "tcp",
    }],
    desc: "${device}",
    shared_stack_stub: true,
    object: {
        shared_stack: true,
        noc: {
            type: "mongo",
            isolate: "${device}",
        },
        listener: [
            {
                type: "http-bdt",
                vport: "84",
            },
            {
                type: "http",
                listen: "127.0.0.1:${http_port}",
            },
            {
                type: "ws",
                enable: true,
                listen: "127.0.0.1:${ws_port}"
            }
        ]
    }
}]
"#;

/*
对于索引为1的stack，需要注意对应的文件所有变化：
1. device1.desc+device1.sec在cyfs/etc/desc/目录下，使用索引区分
    比如索引1对应device1.desc+device1.sec
2. noc数据库文件在cyfs/data/device1/目录下面
*/

pub(crate) struct CyfsStackParam {
    pub device: String,
    pub bdt_port: u16,
    pub http_port: u16,
    pub ws_port: u16,
}

pub(crate) struct CyfsStackHolderImpl {
    param: CyfsStackParam,

    config: Option<CyfsServiceLoaderConfig>,

    cyfs_stack: Option<CyfsStack>,
    shared_stack: Option<SharedCyfsStack>,
}

impl CyfsStackHolderImpl {
    pub fn new(param: CyfsStackParam) -> Self {
        Self {
            param,
            config: None,
            cyfs_stack: None,
            shared_stack: None,
        }
    }

    pub fn load(&mut self) -> BuckyResult<()> {
        let ret = STACK_CONFIG
            .replace("${device}", &self.param.device)
            .replace("${bdt_port}", &self.param.bdt_port.to_string())
            .replace("${http_port}", &self.param.http_port.to_string())
            .replace("${ws_port}", &self.param.ws_port.to_string());

        info!("will use non stack config: {}", ret);
        let config = CyfsServiceLoaderConfig::new_from_string(&ret)?;
        self.config = Some(config);
        Ok(())
    }

    async fn init_stack(&mut self) -> BuckyResult<()> {
        assert!(self.config.is_some());
        let config = self.config.take().unwrap();

        if let Err(e) = CyfsServiceLoader::direct_load(config).await {
            error!("load non stack failed! err={}", e);
            return Err(e);
        }

        assert!(self.cyfs_stack.is_none());
        let stack = CyfsServiceLoader::cyfs_stack(Some(&self.param.device));
        self.cyfs_stack = Some(stack);

        let shared_stack = CyfsServiceLoader::shared_cyfs_stack(Some(&self.param.device));
        self.shared_stack = Some(shared_stack);

        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct CyfsStackHolder(Arc<Mutex<CyfsStackHolderImpl>>);

impl CyfsStackHolder {
    pub fn new(param: CyfsStackParam) -> Self {
        Self(Arc::new(Mutex::new(CyfsStackHolderImpl::new(param))))
    }

    pub async fn start(&self) -> BuckyResult<()> {
        let mut stack = self.0.lock().unwrap();
        stack.load()?;

        stack.init_stack().await?;
        Ok(())
    }

    pub fn shared_stack(&self) -> SharedCyfsStack {
        self.0.lock().unwrap().shared_stack.as_ref().unwrap().clone()
    }
}

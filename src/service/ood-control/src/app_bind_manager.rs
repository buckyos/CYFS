use crate::controller::*;
use crate::interface::*;
use crate::OODControlMode;
use cyfs_base::*;
use cyfs_util::*;

use futures::future::{AbortHandle, Abortable};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
struct BindNotify {
    abort_handle: Arc<Mutex<Option<AbortHandle>>>,
}

impl EventListenerSyncRoutine<(), ()> for BindNotify {
    fn call(&self, _: &()) -> BuckyResult<()> {
        if let Some(abort_handle) = self.abort_handle.lock().unwrap().take() {
            info!("wakeup app on bind");
            abort_handle.abort();
        }
        Ok(())
    }
}

// 适用于内嵌协议栈的
#[derive(Clone)]
pub struct AppBindManager {
    controller: Controller,
    control_interface: Arc<ControlInterface>,
    abort_handle: Arc<Mutex<Option<AbortHandle>>>,
}

impl AppBindManager {
    pub fn new(tcp_port: Option<u16>) -> Self {
        let controller = Controller::new();
        let param = ControlInterfaceParam {
            mode: OODControlMode::App,
            tcp_port,
            require_access_token: false,
            tcp_host: None,
            addr_type: ControlInterfaceAddrType::All,
        };

        let control_interface = ControlInterface::new(param, &controller);

        Self {
            controller,
            control_interface: Arc::new(control_interface),
            abort_handle: Arc::new(Mutex::new(None)),
        }
    }

    pub fn is_bind(&self) -> bool {
        self.controller.is_bind()
    }

    // start成功后，返回本地tcp的绑定端口，用以展示二维码等操作
    pub fn get_tcp_addr_list(&self) -> Vec<SocketAddr> {
        self.control_interface.get_tcp_addr_list()
    }

    //启动本地绑定服务器，必须is_bind返回false才可以调用！
    pub async fn start(&self) -> BuckyResult<()> {
        if self.is_bind() {
            return Ok(());
        }
        //assert!(!self.is_bind());

        self.control_interface.start().await
    }

    // 不等待绑定完成，直接停止整个绑定流程
    pub fn stop(&self) {
        if let Some(handle) = self.abort_handle.lock().unwrap().take() {
            handle.abort();
        }

        self.control_interface.stop();
    }

    // start成功后，等待绑定完成
    pub async fn wait_util_bind(&self) -> BuckyResult<()> {
        if self.controller.is_bind() {
            return Ok(());
        }

        let (abort_handle, abort_registration) = AbortHandle::new_pair();

        // 保留abort—handle用以取消
        {
            let mut handle = self.abort_handle.lock().unwrap();
            assert!(handle.is_none());
            *handle = Some(abort_handle);
        }

        // 关注绑定事件
        let notify = BindNotify {
            abort_handle: self.abort_handle.clone(),
        };

        self.controller.bind_event().on(Box::new(notify.clone()));

        // 等待绑定结束
        match Abortable::new(async_std::future::pending::<()>(), abort_registration).await {
            Ok(_) => {
                unreachable!();
            }
            Err(futures::future::Aborted { .. }) => {
                info!("app bind wakeup on bind! now will stop");
            }
        }

        self.control_interface.stop();

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::*;

    #[cfg(test)]
    #[async_std::test]
    async fn test_bind_manager() {
        let binder = AppBindManager::new(Some(1888));
        let binder2 = binder.clone();
        binder.start().await.unwrap();

        async_std::task::spawn(async move {
            async_std::task::sleep(std::time::Duration::from_secs(5)).await;

            binder2.stop();
        });
        
        binder.wait_util_bind().await;
        async_std::task::sleep(std::time::Duration::from_secs(10)).await;
    }
}
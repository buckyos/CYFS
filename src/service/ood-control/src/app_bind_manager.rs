use crate::controller::*;
use crate::interface::*;
use crate::OODControlMode;
use cyfs_base::*;
use cyfs_util::*;

use futures::future::{AbortHandle, Abortable};
use std::sync::Arc;
use cyfs_debug::Mutex;

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

// For embedded cyfs-stack
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

    // After a successful start, the local tcp bind port is returned, which can be used to display QR codes and other operations.
    pub fn get_tcp_addr_list(&self) -> Vec<SocketAddr> {
        self.control_interface.get_tcp_addr_list()
    }

    // Start local binding server, must be called only if is_bind returns false!
    pub async fn start(&self) -> BuckyResult<()> {
        if self.is_bind() {
            return Ok(());
        }
        //assert!(!self.is_bind());

        self.control_interface.start().await
    }

    // Stop the whole binding process directly without waiting for the binding to complete
    pub fn stop(&self) {
        if let Some(handle) = self.abort_handle.lock().unwrap().take() {
            handle.abort();
        }

        self.control_interface.stop();
    }

    // After a successful start, wait for the binding to complete
    pub async fn wait_util_bind(&self) -> BuckyResult<()> {
        if self.controller.is_bind() {
            return Ok(());
        }

        let (abort_handle, abort_registration) = AbortHandle::new_pair();

        // Keep abort-handle for later cancellation
        {
            let mut handle = self.abort_handle.lock().unwrap();
            assert!(handle.is_none());
            *handle = Some(abort_handle);
        }

        // Focus on the binding events
        let notify = BindNotify {
            abort_handle: self.abort_handle.clone(),
        };

        self.controller.bind_event().on(Box::new(notify.clone()));

        // Wait for binding to end
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
        
        let r = binder.wait_util_bind().await;
        if let Err(e) = r {
            println!("bind error: {}", e);
        } else {
            println!("bind success");
        }
        
        async_std::task::sleep(std::time::Duration::from_secs(10)).await;
    }
}
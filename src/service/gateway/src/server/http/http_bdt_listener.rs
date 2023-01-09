use async_std::stream::StreamExt;
use async_std::task;
use futures::future::{self, AbortHandle, Aborted};
use http_types::{Response, StatusCode};
use std::sync::{Arc, Mutex};

use super::http_listener_base::HttpListenerBase;
use base::{ListenerUtil, STACK_MANAGER};
use cyfs_base::BuckyError;
use cyfs_bdt::StreamGuard as BdtStream;

#[derive(Debug)]
pub(crate) struct ServerRunningState {
    pub running: bool,
    pub canceler: Option<AbortHandle>,
}

impl ServerRunningState {
    pub fn new() -> Self {
        Self {
            running: false,
            canceler: None,
        }
    }
}
#[derive(Debug, Clone)]
pub(super) struct HttpBdtListener {
    pub stack: String,
    pub vport: u16,
    base: HttpListenerBase,

    state: Arc<Mutex<ServerRunningState>>,
}

impl HttpBdtListener {
    pub fn new() -> HttpBdtListener {
        HttpBdtListener {
            vport: 0u16,
            stack: String::from(""),
            base: HttpListenerBase::new(),

            state: Arc::new(Mutex::new(ServerRunningState::new())),
        }
    }

    pub fn is_running(&self) -> bool {
        self.state.lock().unwrap().running
    }

    pub fn bind_forward(&self, forward_id: u32) {
        info!(
            "http bdt listener bind new forward: listener={}:{}, forward_id={}",
            self.stack, self.vport, forward_id
        );

        self.base.bind_forward(forward_id);
    }

    pub fn unbind_forward(&self, forward_id: u32) -> bool {
        if self.base.unbind_forward(forward_id) {
            info!(
                "http bdt listener unbind forward: listener={}:{}, forward_id={}",
                self.stack, self.vport, forward_id
            );
            true
        } else {
            false
        }
    }

    pub fn load(&self, _server_node: &toml::value::Table) -> Result<(), BuckyError> {
        Ok(())
    }

    pub async fn run(&self) -> Result<(), BuckyError> {
        {
            let mut state = self.state.lock().unwrap();
            // 这里判断一次状态
            if state.running {
                warn!(
                    "http bdt listener already running! listen={}:{}",
                    self.stack, self.vport
                );
                return Ok(());
            }

            state.running = true;
        }

        let ret = self.run_inner().await;

        self.state.lock().unwrap().running = false;

        ret
    }

    async fn run_inner(&self) -> Result<(), BuckyError> {
        let stack;
        let addr;
        let listen = format!("({}:{})", self.stack, self.vport);

        {
            let stack_item = STACK_MANAGER.get_bdt_stack(Some(&self.stack));
            if stack_item.is_none() {
                return BuckyError::error_with_log(format!(
                    "bdt server stack not found! stack={}",
                    self.stack
                ));
            }

            stack = stack_item.unwrap();
            let local_addr = STACK_MANAGER
                .get_bdt_stack_local_addr(Some(&self.stack))
                .unwrap();
            addr = format!("http://{}", local_addr);
        }

        let bdt_listener = stack.stream_manager().listen(self.vport);
        if let Err(e) = bdt_listener {
            error!(
                "http bdt listen error: stack={}, {}:{} {}",
                self.stack, addr, self.vport, e
            );
            return Err(e);
        } else {
            info!(
                "http bdt listen: stack={}, {}:{}",
                self.stack, addr, self.vport
            );
        }

        let listen2 = listen.clone();

        let (future, handle) = future::abortable(async move {
            let bdt_listener = bdt_listener.unwrap();

            let mut incoming = bdt_listener.incoming();
            loop {
                let incoming_ret = incoming.next().await;
                match incoming_ret {
                    Some(Ok(pre_stream)) => {
                        info!("recv new bdt connection: {:?}", pre_stream.stream.remote());

                        let this = self.clone();
                        let addr = addr.clone();
                        task::spawn(async move {
                            if let Err(e) = this.accept(addr, pre_stream.stream).await {
                                error!("process stream error: err={}", e);
                            }
                        });
                    }
                    Some(Err(e)) => {
                        // FIXME 返回错误后如何处理？是否要停止
                        error!(
                            "recv bdt http connection error! listener={}, err={}",
                            listen, e
                        );
                    }
                    None => {
                        info!("bdt http listener finished! listener={}", listen);
                        break;
                    }
                }
            }
        });

        // 保存abort_handle
        {
            let mut state = self.state.lock().unwrap();
            assert!(state.canceler.is_none());
            state.canceler = Some(handle);
        }

        match future.await {
            Ok(_) => {
                info!(
                    "http bdt listener recv incoming finished complete: {}",
                    listen2
                );

                let mut state = self.state.lock().unwrap();
                state.canceler = None;
                state.running = false;
            }
            Err(Aborted) => {
                info!("http bdt listener recv incoming aborted: {}", listen2);
            }
        };

        Ok(())
    }

    pub fn stop(&self) {
        let ret = {
            let mut state = self.state.lock().unwrap();
            state.running = false;
            state.canceler.take()
        };

        if let Some(abort) = ret {
            info!("will stop http bdt listener: {}:{}", self.stack, self.vport);
            abort.abort();
        }
    }

    async fn accept(&self, addr: String, stream: BdtStream) -> Result<(), BuckyError> {
        let remote_addr = stream.remote();
        let remote_addr = (remote_addr.0.clone(), remote_addr.1);
        info!(
            "starting accept new bdt connection at {} from {:?}",
            addr, remote_addr
        );

        if let Err(e) = stream.confirm(&vec![]).await {
            error!("bdt stream confirm error! {:?}, {}", remote_addr, e,);
            return Err(e);
        }

        let device_id = &remote_addr.0;
        let opts = async_h1::ServerOptions::default();
        let ret = async_h1::accept_with_opts(
            stream,
            |mut req| async move {
                info!("recv bdt http request: {:?}", req);

                {
                    let state = self.state.lock().unwrap();
                    if !state.running {
                        error!(
                            "bdt http server already closed, server=({}, {})",
                            self.stack, self.vport
                        );
                        return Ok(Response::new(StatusCode::InternalServerError));
                    }
                }

                // req插入remote_peer_id头部
                // 注意这里用insert而不是append，防止用户自带此header导致错误peerid被结算攻击
                req.insert_header(cyfs_base::CYFS_REMOTE_DEVICE, device_id.to_string());

                let resp = self.base.dispatch_request(req).await;
                Ok(resp)
            },
            opts,
        )
        .await;

        if let Err(e) = ret {
            error!(
                "accept error, err={}, addr={}, device={:?}",
                e, addr, remote_addr
            );
            return Err(BuckyError::from(e));
        }

        Ok(())
    }
}

pub(super) struct HttpBdtListenerManager {
    server_list: Mutex<Vec<Arc<HttpBdtListener>>>,
}

impl HttpBdtListenerManager {
    pub fn new() -> HttpBdtListenerManager {
        HttpBdtListenerManager {
            server_list: Mutex::new(Vec::new()),
        }
    }

    /*
    {
        type: "bdt",
        stack: "bdt_public",
        vport: 80,
    }
    */
    pub fn load(
        &self,
        server_node: &toml::value::Table,
    ) -> Result<Arc<HttpBdtListener>, BuckyError> {
        let (stack, vport) = ListenerUtil::load_bdt_listener(server_node)?;

        let item = self.get_or_create(stack.as_str(), vport);
        if let Err(e) = item.load(server_node) {
            error!(
                "load bdt listener failed! err={}, node={:?}",
                e, server_node
            );
        }

        return Ok(item);
    }

    fn get_or_create(&self, stack: &str, vport: u16) -> Arc<HttpBdtListener> {
        {
            let mut list = self.server_list.lock().unwrap();
            let ret = list
                .iter()
                .any(|item| item.stack == stack && item.vport == vport);

            if !ret {
                let mut server = HttpBdtListener::new();
                server.stack = stack.to_owned();
                server.vport = vport;

                let server = Arc::new(server);
                list.push(server);
            }
        }

        self.get_item(stack, vport).unwrap()
    }

    pub fn get_item(&self, stack: &str, vport: u16) -> Option<Arc<HttpBdtListener>> {
        let list = self.server_list.lock().unwrap();
        for item in &*list {
            if item.stack == stack && item.vport == vport {
                return Some(item.clone());
            }
        }

        return None;
    }

    pub fn unbind_forward(&self, forward_id: u32) {
        let list = self.server_list.lock().unwrap();
        for server in &*list {
            server.unbind_forward(forward_id);

            // 如果没有绑定任何转发器，那么停止该listener
            if server.base.forward_count() == 0 {
                server.stop();
            }
        }
    }

    pub fn start(&self) {
        let list = self.server_list.lock().unwrap();
        for server in &*list {
            if !server.is_running() {
                let server = server.clone();
                task::spawn(async move {
                    let _r = server.run().await;
                });
            }
        }
    }

    /*
    pub fn stop_idle(&self) {
        for server in &self.server_list {
            let mut server = server.lock().unwrap();
            if server.base.lock().unwrap().forward_count() == 0 {
                server.stop();
            }
        }
    }
    */
}

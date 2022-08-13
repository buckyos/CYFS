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
pub(super) struct HttpBdtListener {
    pub stack: String,
    pub vport: u16,
    base: Arc<Mutex<HttpListenerBase>>,

    running: bool,
    canceler: Option<AbortHandle>,
}

impl HttpBdtListener {
    pub fn new() -> HttpBdtListener {
        HttpBdtListener {
            vport: 0u16,
            stack: String::from(""),
            base: Arc::new(Mutex::new(HttpListenerBase::new())),

            running: false,
            canceler: None,
        }
    }

    pub fn bind_forward(&self, forward_id: u32) {
        info!(
            "http bdt listener bind new forward: listener={}:{}, forward_id={}",
            self.stack, self.vport, forward_id
        );

        let mut base = self.base.lock().unwrap();
        base.bind_forward(forward_id);
    }

    pub fn unbind_forward(&self, forward_id: u32) -> bool {
        let mut base = self.base.lock().unwrap();
        if base.unbind_forward(forward_id) {
            info!(
                "http bdt listener unbind forward: listener={}:{}, forward_id={}",
                self.stack, self.vport, forward_id
            );
            true
        } else {
            false
        }
    }

    pub fn load(
        &mut self,
        _server_node: &toml::value::Table,
    ) -> Result<(), BuckyError> {
        Ok(())
    }

    pub async fn run(listener: Arc<Mutex<HttpBdtListener>>) -> Result<(), BuckyError> {
        {
            let mut listener = listener.lock().unwrap();
            // 这里判断一次状态
            if listener.running {
                warn!(
                    "http bdt listener already running! listen={}:{}",
                    listener.stack, listener.vport
                );
                return Ok(());
            }

            listener.running = true;
        }

        let ret = Self::run_inner(listener.clone()).await;

        listener.lock().unwrap().running = false;

        ret
    }

    async fn run_inner(listener: Arc<Mutex<HttpBdtListener>>) -> Result<(), BuckyError> {
        let stack_id;
        let vport;
        {
            let listener = listener.lock().unwrap();
            stack_id = listener.stack.clone();
            vport = listener.vport;
        }

        let stack;
        let addr;
        let listen = format!("({}:{})", stack_id, vport);

        {
            let stack_item = STACK_MANAGER.get_bdt_stack(Some(&stack_id));
            if stack_item.is_none() {
                return BuckyError::error_with_log(format!(
                    "bdt server stack not found! stack={}",
                    stack_id
                ));
            }

            stack = stack_item.unwrap();
            let local_addr = STACK_MANAGER.get_bdt_stack_local_addr(Some(&stack_id)).unwrap();
            addr = format!("http://{}", local_addr);
        }

        let bdt_listener = stack.stream_manager().listen(vport);
        if let Err(e) = bdt_listener {
            error!(
                "http bdt listen error: stack={}, {}:{} {}",
                stack_id, addr, vport, e
            );
            return Err(e);
        } else {
            info!("http bdt listen: stack={}, {}:{}", stack_id, addr, vport);
        }

        let listen2 = listen.clone();
        let listener2 = listener.clone();

        let (future, handle) = future::abortable(async move {
            let bdt_listener = bdt_listener.unwrap();

            let mut incoming = bdt_listener.incoming();
            loop {
                let incoming_ret = incoming.next().await;
                match incoming_ret {
                    Some(Ok(pre_stream)) => {
                        info!("recv new bdt connection: {:?}", pre_stream.stream.remote());

                        let listener = listener.clone();
                        let addr = addr.clone();
                        task::spawn(async move {
                            if let Err(e) = Self::accept(&listener, addr, pre_stream.stream).await {
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
            let mut listener = listener2.lock().unwrap();
            assert!(listener.canceler.is_none());
            listener.canceler = Some(handle);
        }

        match future.await {
            Ok(_) => {
                info!(
                    "http bdt listener recv incoming finished complete: {}",
                    listen2
                );

                let mut listener = listener2.lock().unwrap();
                listener.canceler = None;
                listener.running = false;
            }
            Err(Aborted) => {
                info!("http bdt listener recv incoming aborted: {}", listen2);
            }
        };

        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(abort) = self.canceler.take() {
            info!("will stop http bdt listener: {}:{}", self.stack, self.vport);
            abort.abort();
        }

        self.running = false;
    }

    async fn accept(
        listener: &Arc<Mutex<HttpBdtListener>>,
        addr: String,
        stream: BdtStream,
    ) -> Result<(), BuckyError> {
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
        let ret = async_h1::accept_with_opts(stream, |mut req| async move {
            info!("recv bdt http request: {:?}", req);

            let base;
            {
                let server = listener.lock().unwrap();
                if server.running {
                    base = server.base.clone();
                } else {
                    error!(
                        "bdt http server already closed, server=({}, {})",
                        server.stack, server.vport
                    );
                    return Ok(Response::new(StatusCode::InternalServerError));
                }
            }

            // req插入remote_peer_id头部
            // 注意这里用insert而不是append，防止用户自带此header导致错误peerid被结算攻击
            req.insert_header(cyfs_base::CYFS_REMOTE_DEVICE, device_id.to_string());

            let resp = HttpListenerBase::dispatch_request(&base, req).await;
            Ok(resp)
        }, opts)
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
    server_list: Vec<Arc<Mutex<HttpBdtListener>>>,
}

impl HttpBdtListenerManager {
    pub fn new() -> HttpBdtListenerManager {
        HttpBdtListenerManager {
            server_list: Vec::new(),
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
        &mut self,
        server_node: &toml::value::Table,
    ) -> Result<Arc<Mutex<HttpBdtListener>>, BuckyError> {
        let (stack, vport) = ListenerUtil::load_bdt_listener(server_node)?;

        let listener = self.get_or_create(stack.as_str(), vport);
        {
            let mut item = listener.lock().unwrap();
            if let Err(e) = item.load(server_node) {
                error!(
                    "load bdt listener failed! err={}, node={:?}",
                    e, server_node
                );
            }
        }

        return Ok(listener);
    }

    fn get_or_create(&mut self, stack: &str, vport: u16) -> Arc<Mutex<HttpBdtListener>> {
        let ret = self.server_list.iter().any(|item| {
            let item = item.lock().unwrap();
            item.stack == stack && item.vport == vport
        });

        if !ret {
            let mut server = HttpBdtListener::new();
            server.stack = stack.to_owned();
            server.vport = vport;

            let server = Arc::new(Mutex::new(server));
            self.server_list.push(server);
        }

        return self.get_item(stack, vport).unwrap();
    }

    pub fn get_item(&self, stack: &str, vport: u16) -> Option<Arc<Mutex<HttpBdtListener>>> {
        for server in &self.server_list {
            let item = server.lock().unwrap();
            if item.stack == stack && item.vport == vport {
                return Some(server.clone());
            }
        }

        return None;
    }

    pub fn unbind_forward(&self, forward_id: u32) {
        for server in &self.server_list {
            let mut server = server.lock().unwrap();
            server.unbind_forward(forward_id);

            // 如果没有绑定任何转发器，那么停止该listener
            if server.base.lock().unwrap().forward_count() == 0 {
                server.stop();
            }
        }
    }

    pub fn start(&self) {
        for server in &self.server_list {
            if !server.lock().unwrap().running {
                let server = server.clone();
                task::spawn(async move {
                    let _r = HttpBdtListener::run(server).await;
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

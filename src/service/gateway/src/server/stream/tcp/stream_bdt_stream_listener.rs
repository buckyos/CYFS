use crate::upstream::TcpUpStreamForBdt;
use base::STACK_MANAGER;
use cyfs_base::BuckyError;
use cyfs_bdt::StreamGuard as BdtStream;

use async_std::stream::StreamExt;
use async_std::task;
use futures::future::{self, AbortHandle, Aborted};
use std::sync::{Arc, Mutex};

pub struct StreamBdtListener {
    pub stack: String,
    pub vport: u16,
    proxy_pass: (String, u16),

    pub running: bool,
    canceler: Option<AbortHandle>,
}

impl StreamBdtListener {
    pub fn new(listener: (String, u16)) -> Self {
        Self {
            stack: listener.0,
            vport: listener.1,
            proxy_pass: ("".to_owned(), 0),

            running: false,
            canceler: None,
        }
    }

    pub fn bind_proxy_pass(&mut self, proxy_pass: &(String, u16)) {
        assert!(proxy_pass.0.len() > 0);
        assert!(proxy_pass.1 > 0);

        self.proxy_pass = proxy_pass.clone();
    }

    pub fn stop(listener: &Arc<Mutex<StreamBdtListener>>) {
        let mut listener = listener.lock().unwrap();

        if let Some(abort) = listener.canceler.take() {
            info!(
                "will stop bdt stream server {}:{}",
                listener.stack, listener.vport
            );
            abort.abort();
        }

        listener.running = false;
    }

    pub async fn run(listener: Arc<Mutex<StreamBdtListener>>) -> Result<(), BuckyError> {
        {
            let mut listener = listener.lock().unwrap();

            // 这里判断一次状态
            if listener.running {
                warn!(
                    "stream bdt stream listener already running! listen={}:{}",
                    listener.stack, listener.vport
                );
                return Ok(());
            }

            // 标记为运行状态
            listener.running = true;
            assert!(listener.canceler.is_none());
        }

        let ret = Self::run_inner(listener.clone()).await;

        listener.lock().unwrap().running = false;

        ret
    }

    async fn run_inner(listener: Arc<Mutex<StreamBdtListener>>) -> Result<(), BuckyError> {
        let stack_name;
        let vport;
        let proxy_pass;
        {
            let listener = listener.lock().unwrap();

            stack_name = listener.stack.clone();
            vport = listener.vport;
            proxy_pass = listener.proxy_pass.clone();
        }

        let listen = format!("{}:{}", stack_name, vport);

        let bdt_addr;
        let bdt_stack;
        {
            let stack_item = STACK_MANAGER.get_bdt_stack(Some(&stack_name));
            if stack_item.is_none() {
                return BuckyError::error_with_log(format!(
                    "bdt server stack not found! stack={}",
                    stack_name
                ));
            }

            bdt_stack = stack_item.unwrap();
            let local_addr = STACK_MANAGER.get_bdt_stack_local_addr(Some(&stack_name)).unwrap();
            bdt_addr = format!("http://{}", local_addr);
        }

        let bdt_listener = bdt_stack.stream_manager().listen(vport);
        if let Err(e) = bdt_listener {
            error!("stream bdt listen error: {} {}", listen, e);
            return Err(e);
        } else {
            info!("stream bdt listen at {} {}", listen, bdt_addr);
        }

        let listen2 = listen.clone();
        // let listener2 = listener.clone();

        let (future, handle) = future::abortable(async move {
            let bdt_listener = bdt_listener.unwrap();
            let mut incoming = bdt_listener.incoming();

            loop {
                let incoming_ret = incoming.next().await;
                match incoming_ret {
                    Some(v) => match v {
                        Ok(pre_stream) => {
                            info!(
                                "recv new bdt connection, listen={}, remote={:?}, proxy_pass={:?}",
                                listen2,
                                pre_stream.stream.remote(),
                                proxy_pass
                            );

                            let address = (proxy_pass.0.clone(), proxy_pass.1 as u32);
                            task::spawn(async move {
                                Self::process(address, pre_stream.stream).await;
                            });
                        }
                        Err(e) => {
                            // FIXME 这里出错后如何处理？是否需要终止
                            error!(
                                "bdt stream listener accept error, listen={}, err={:?}",
                                listen2, e
                            );
                        }
                    },
                    None => {
                        info!("bdt stream incoming finished: listen={}", listen2);

                        break;
                    }
                }
            }
        });

        // 保存abort_handle
        {
            let mut listener = listener.lock().unwrap();
            assert!(listener.canceler.is_none());
            listener.canceler = Some(handle);
        }

        match future.await {
            Ok(_) => {
                info!("bdt stream recv incoming finished complete: {}", listen);

                let mut listener = listener.lock().unwrap();
                listener.canceler = None;
                listener.running = false;
            }
            Err(Aborted) => {
                info!("bdt stream recv incoming aborted: {}", listen);
            }
        };

        Ok(())
    }

    async fn process(proxy_pass: (String, u32), stream: BdtStream) {
        if let Err(e) = stream.confirm(&Vec::from("answer")).await {
            error!(
                "bdt stream confirm error! proxy_pass={:?}, remote={:?}, {}",
                proxy_pass,
                stream.remote(),
                e,
            );
            return;
        }

        let upstream = TcpUpStreamForBdt::new(&proxy_pass);

        let remote = format!("{:?}", stream.remote());
        if let Err(e) = upstream.bind(stream).await {
            error!("bind bdt stream error: remote={}, err={}", remote, e);
        }
    }
}

pub struct StreamBdtListenerManager {
    server_list: Vec<Arc<Mutex<StreamBdtListener>>>,
}

impl StreamBdtListenerManager {
    pub fn new() -> StreamBdtListenerManager {
        StreamBdtListenerManager {
            server_list: Vec::new(),
        }
    }

    pub fn bind_proxy_pass(&mut self, proxy_pass: &(String, u16)) {
        for server in &self.server_list {
            let mut server = server.lock().unwrap();
            server.bind_proxy_pass(proxy_pass);
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
    ) -> Result<(), BuckyError> {
        let (stack, vport) = match ::base::ListenerUtil::load_bdt_listener(server_node) {
            Ok(v) => v,
            Err(e) => {
                return Err(e);
            }
        };

        // 检查是否已经存在相同的stack+vport
        let ret = self.server_list.iter().any(|item| {
            let item = item.lock().unwrap();
            item.stack == stack && item.vport == vport
        });
        if ret {
            let msg = format!(
                "tcp stream's bdt listener already exists! stack={}, vport={}",
                stack, vport
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        let server = StreamBdtListener::new((stack.to_owned(), vport));
        let server = Arc::new(Mutex::new(server));
        self.server_list.push(server);

        return Ok(());
    }

    pub fn start(&self) {
        for server in &self.server_list {
            let server = server.clone();
            if !server.lock().unwrap().running {
                task::spawn(async move {
                    let _r = StreamBdtListener::run(server).await;
                });
            }
        }
    }

    pub fn stop(&self) {
        for server in &self.server_list {
            StreamBdtListener::stop(server);
        }
    }
}

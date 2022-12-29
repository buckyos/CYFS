use crate::upstream::{UdpUpStreamManager, UpstreamDatagramSender};
use cyfs_stack_loader::STACK_MANAGER;
use cyfs_base::BuckyError;
use cyfs_stack_loader::ListenerUtil;

use async_std::task;
use futures::future::{AbortHandle};
use std::sync::{Arc, Mutex};

pub struct DatagramBdtListener {
    pub stack: String,
    pub vport: u16,
    proxy_pass: (String, u16),

    pub running: bool,
    canceler: Option<AbortHandle>,
}

impl DatagramBdtListener {
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

    pub fn stop(listener: &Arc<Mutex<DatagramBdtListener>>) {
        let mut listener = listener.lock().unwrap();

        if let Some(abort) = listener.canceler.take() {
            info!(
                "will stop bdt datagram server {}:{}",
                listener.stack, listener.vport
            );
            abort.abort();
        }

        listener.running = false;
    }

    pub async fn run(listener: Arc<Mutex<DatagramBdtListener>>) -> Result<(), BuckyError> {
        {
            let mut listener = listener.lock().unwrap();

            // 这里判断一次状态
            if listener.running {
                warn!(
                    "stream bdt datagram listener already running! listen={}:{}",
                    listener.stack, listener.vport
                );
                return Ok(());
            }

            // 标记为运行状态
            listener.running = true;
        }

        let ret = Self::run_inner(listener.clone()).await;

        listener.lock().unwrap().running = false;

        ret
    }

    async fn run_inner(listener: Arc<Mutex<DatagramBdtListener>>) -> Result<(), BuckyError> {
        let stack_name;
        let vport;
        let proxy_pass;
        {
            let listener = listener.lock().unwrap();

            stack_name = listener.stack.clone();
            vport = listener.vport;
            proxy_pass = listener.proxy_pass.clone();
        }

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
        }

        let bdt_channel = bdt_stack.datagram_manager().bind(vport);
        if let Err(e) = bdt_channel {
            error!(
                "create bdt datagram tunnel error! at={}:{}, {}",
                stack_name, vport, e
            );

            return Err(e);
        } else {
            info!(
                "create bdt datagram tunnel success! {}:{}",
                stack_name, vport
            );
        }

        let bdt_channel = bdt_channel.unwrap();
        let _bdt_channel2 = bdt_channel.clone();
        let _sender = bdt_channel.clone_sender();

        let channel_addr = format!("{}:{}", stack_name, vport);
        let _channel_addr2 = channel_addr.clone();
        let proxy_pass_str = format!("{}:{}", proxy_pass.0, proxy_pass.1);
        let mut upstream_manager = UdpUpStreamManager::new(&channel_addr, &proxy_pass_str);
        upstream_manager.start();
        /*
        let (future, handle) = future::abortable(async move {
            let mut buf: Vec<u8> = Vec::with_capacity(1600);
            buf.resize(1600, 0);

            loop {
                let ret = bdt_channel.recv_from(&mut buf).await;
                match ret {
                    Ok((len, source)) => {
                        let src_addr = format!("{}:{}", source.remote, source.vport);
                        trace!(
                            "bdt channel recv data from: {}, {:?}",
                            src_addr, source.zone
                        );

                        match upstream_manager
                            .pick_stream(&src_addr, &proxy_pass, &sender, Some(&source.remote))
                            .await
                        {
                            Ok(stream) => match stream.send(&buf[..len]).await {
                                Ok(send_len) => {
                                    debug!(
                                        "forward udp package: {} -> {}, send_len={}",
                                        src_addr, proxy_pass_str, send_len
                                    );
                                    assert!(send_len == len);
                                }
                                Err(e) => error!(
                                    "send package to upstream error! proxy_pass={}, err={}",
                                    proxy_pass_str, e
                                ),
                            },
                            Err(e) => {
                                error!(
                                    "pick udp upstram error! proxy_pass={:?}, src_addr={}, err={}",
                                    proxy_pass, src_addr, e
                                );
                            }
                        };
                    }
                    Err(e) => {
                        // FIXME 这里出错后如何处理？是否需要终止
                        error!(
                            "bdt datagram tunnel recv error, listener=({}), err={:?}",
                            channel_addr, e
                        );
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
                info!(
                    "bdt datagram recv incoming finished complete: {}",
                    channel_addr2
                );

                let mut listener = listener.lock().unwrap();
                listener.canceler = None;
                listener.running = false;
            }
            Err(Aborted) => {
                info!("bdt datagram recv incoming aborted: {}", channel_addr2);
            }
        };

        bdt_channel2.close();
        
        */
        Ok(())
    }
}

pub struct DatagramBdtListenerManager {
    server_list: Vec<Arc<Mutex<DatagramBdtListener>>>,
}

impl DatagramBdtListenerManager {
    pub fn new() -> DatagramBdtListenerManager {
        DatagramBdtListenerManager {
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
        stack: "default",
        vport: 80,
    }
    */
    pub fn load(
        &mut self,
        server_node: &toml::value::Table,
    ) -> Result<(), BuckyError> {
        let (stack, vport) = match ListenerUtil::load_bdt_listener(server_node) {
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
                "udp stream's bdt listener already exists! stack={}, vport={}",
                stack, vport
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        let server = DatagramBdtListener::new((stack.to_owned(), vport));
        let server = Arc::new(Mutex::new(server));
        self.server_list.push(server);

        return Ok(());
    }

    pub fn start(&self) {
        for server in &self.server_list {
            let server = server.clone();
            if !server.lock().unwrap().running {
                task::spawn(async move {
                    let _r = DatagramBdtListener::run(server).await;
                });
            }
        }
    }

    pub fn stop(&self) {
        for server in &self.server_list {
            DatagramBdtListener::stop(server);
        }
    }
}
use cyfs_base::{BuckyError, BuckyResult};
use cyfs_debug::Mutex;

use async_std::net::{TcpListener, TcpStream};
use async_std::stream::StreamExt;
use async_std::task;
use futures::future::{AbortHandle, Aborted};
use std::net::SocketAddr;
use std::sync::Arc;
use once_cell::sync::OnceCell;


#[async_trait::async_trait]
pub trait BaseTcpListenerHandler: Send + Sync {
    async fn on_accept(&self, stream: TcpStream) -> BuckyResult<()>;
}

pub type BaseTcpListenerHandlerRef = Arc<Box<dyn BaseTcpListenerHandler>>;

struct BaseTcpListenerInner {
    listen: SocketAddr,

    handler: OnceCell<BaseTcpListenerHandlerRef>,

    // 取消listener的运行
    canceler: Option<AbortHandle>,

    running_task: Option<async_std::task::JoinHandle<()>>,
}

impl BaseTcpListenerInner {
    pub fn new(listen: SocketAddr) -> Self {
        Self {
            listen,
            handler: OnceCell::new(),
            canceler: None,
            running_task: None,
        }
    }

    fn bind_handler(&self,  handler: BaseTcpListenerHandlerRef) {
        if let Err(_) = self.handler.set(handler) {
            unreachable!();
        }
    }

    pub fn get_addr(&self) -> SocketAddr {
        self.listen.clone()
    }

    pub fn get_listen(&self) -> String {
        self.listen.to_string()
    }
}

#[derive(Clone)]
pub struct BaseTcpListener(Arc<Mutex<BaseTcpListenerInner>>);

impl BaseTcpListener {
    pub fn new(addr: SocketAddr) -> Self {
        let inner = BaseTcpListenerInner::new(addr);

        Self(Arc::new(Mutex::new(inner)))
    }

    
    pub fn bind_handler(&self,  handler: BaseTcpListenerHandlerRef) {
        self.0.lock().unwrap().bind_handler(handler)
    }

    pub fn get_addr(&self) -> SocketAddr {
        self.0.lock().unwrap().get_addr()
    }
    pub fn get_listen(&self) -> String {
        self.0.lock().unwrap().get_listen()
    }

    pub async fn start(&self) -> BuckyResult<()> {
        let tcp_listener = self.create_listener().await?;

        let this = self.clone();
        let (release_task, handle) = futures::future::abortable(async move {
            let _ = this.run_inner(tcp_listener).await;
        });

        let this = self.clone();
        let task = async_std::task::spawn(async move {
            match release_task.await {
                Ok(_) => {
                    info!("tcp listener complete: {}", this.get_listen());
                }
                Err(Aborted) => {
                    info!("tcp listener cancelled: {}", this.get_listen());
                }
            }
        });

        {
            let mut listener = self.0.lock().unwrap();
            assert!(listener.canceler.is_none());
            assert!(listener.running_task.is_none());
            listener.canceler = Some(handle);
            listener.running_task = Some(task);
        }

        Ok(())
    }

    pub async fn stop(&self) {
        let canceler;
        let running_task;

        info!("will stop tcp listener: {}", self.get_listen());

        {
            let mut listener = self.0.lock().unwrap();
            canceler = listener.canceler.take();
            running_task = listener.running_task.take();
        }

        if let Some(canceler) = canceler {
            canceler.abort();
            let running_task = running_task.unwrap();
            running_task.await;
            info!("tcp listener stoped complete: {}", self.get_listen());
        } else {
            warn!("tcp listener not running: {}", self.get_listen());
        }
    }

    async fn create_listener(&self) -> BuckyResult<TcpListener> {
        let listen;
        {
            let listener = self.0.lock().unwrap();
            listen = listener.listen.clone();
        }

        let tcp_listener = TcpListener::bind(listen).await.map_err(|e| {
            let msg = format!(
                "object tcp listener bind addr failed! addr={}, err={}",
                listen, e
            );
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        #[cfg(unix)]
        {
            use async_std::os::unix::io::AsRawFd;
            if let Err(e) = cyfs_util::set_socket_reuseaddr(tcp_listener.as_raw_fd()) {
                error!("set_socket_reuseaddr for {:?} error! err={}", listen, e);
            }
        }

        let local_addr = tcp_listener.local_addr().map_err(|e| {
            error!("get tcp listener local addr failed! {}", e);
            BuckyError::from(e)
        })?;

        // 更新本地的local addr
        {
            let mut listener = self.0.lock().unwrap();
            info!(
                "will update tcp listener local addr: {} -> {}",
                listener.listen, local_addr
            );
            listener.listen = local_addr.clone();
        }

        Ok(tcp_listener)
    }

    async fn run_inner(&self, tcp_listener: TcpListener) -> BuckyResult<()> {
        let listen;
        let handler;
        {
            let listener = self.0.lock().unwrap();

            listen = listener.listen.clone();
            handler = listener.handler.get().unwrap().clone();
        }

        let addr = format!("http://{}", tcp_listener.local_addr().unwrap());
        info!("tcp listener listen at {}", addr);

        let mut incoming = tcp_listener.incoming();
        loop {
            match incoming.next().await {
                Some(Ok(tcp_stream)) => {
                    debug!(
                        "tcp listener recv new connection from {:?}",
                        tcp_stream.peer_addr()
                    );

                    let handler = handler.clone();
                    task::spawn(async move {
                        if let Err(e) = handler.on_accept(tcp_stream).await {
                            error!(
                                "object tcp process http connection error: listen={} err={}",
                                listen, e
                            );
                        }
                    });
                }
                Some(Err(e)) => {
                    // FIXME 返回错误后如何处理？是否要停止
                    let listener = self.0.lock().unwrap();
                    error!(
                        "tcp listener recv connection error! listener={}, err={}",
                        listener.listen, e
                    );
                }
                None => {
                    info!("tcp listener recv connection finished! listen={}", listen);
                    break;
                }
            }
        }

        Ok(())
    }
}

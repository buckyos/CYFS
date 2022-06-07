use super::{AssociationProtocol, PEER_ASSOC_MANAGER};
use cyfs_base::BuckyError;

use async_std::net::TcpStream;
use async_std::prelude::*;
use async_std::task;
use futures::join;
use cyfs_bdt::StreamGuard as BdtStream;
use std::net::Shutdown;
use std::time::Duration;

pub struct TcpUpStream {
    address: (String, u16),
}

impl TcpUpStream {
    pub fn new(address: &(String, u16)) -> TcpUpStream {
        TcpUpStream {
            address: (address.0.clone(), address.1),
        }
    }

    //pub fn new_conn() -> Result<TcpStream, BuckyError> {

    //}

    pub fn init_up_stream(stream: &TcpStream) {
        #[cfg(unix)]
        {
            use async_std::os::unix::io::AsRawFd;
            if let Err(e) = cyfs_util::set_socket_reuseaddr(stream.as_raw_fd()) {
                error!(
                    "set_socket_reuseaddr for {:?} error! err={}",
                    stream.peer_addr(),
                    e
                );
            }

            if let Err(e) = cyfs_util::set_socket_keepalive(stream.as_raw_fd()) {
                error!(
                    "set_socket_keepalive for {:?} error! err={}",
                    stream.peer_addr(),
                    e
                );
            } 
        }

        #[cfg(windows)]
        {
            use async_std::os::windows::io::AsRawSocket;

            if let Err(e) = cyfs_util::set_socket_keepalive(stream.as_raw_socket()) {
                error!(
                    "set_socket_keepalive for {:?} error! err={}",
                    stream.peer_addr(),
                    e
                );
            }
        }
    }

    pub async fn bind(&self, stream: TcpStream) -> Result<(), BuckyError> {
        let str = format!("{}:{}", self.address.0, self.address.1);

        let ret = TcpStream::connect(str).await;
        if let Err(e) = ret {
            error!(
                "connect tcp up stream error, addr={:?}, e={}",
                self.address, e
            );
            return Err(BuckyError::from(e));
        }

        let up_stream = ret.unwrap();
        Self::init_up_stream(&up_stream);

        let up_stream2 = up_stream.clone();
        let stream2 = stream.clone();

        let t1 = task::spawn(async move {
            Self::bind_stream(stream, up_stream).await;
        });

        let t2 = task::spawn(async move {
            Self::bind_stream(up_stream2, stream2).await;
        });

        join!(t1, t2);

        Ok(())
    }

    async fn bind_stream(mut src: TcpStream, mut dest: TcpStream) {
        let mut recv_buf = [0x00_u8; 4096];
        let mut need_clear = true;

        loop {
            let ret = async_std::io::timeout(Duration::from_secs(5), src.read(&mut recv_buf)).await;
            // let ret = src.read(&mut recv_buf).await;
            match ret {
                Ok(recv_size) => {
                    if recv_size > 0 {
                        if let Err(e) = dest.write_all(&recv_buf[0..recv_size]).await {
                            error!(
                                "write to stream error, remote={:?}, err={}",
                                dest.peer_addr(),
                                e
                            );
                            break;
                        }
                    } else {
                        if let Err(e) = dest.shutdown(Shutdown::Write) {
                            error!(
                                "close upstream for write error! remote={:?}， err={}",
                                dest.peer_addr(),
                                e
                            );
                        }

                        need_clear = false;
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == async_std::io::ErrorKind::TimedOut {
                        //  debug!("read timeout, err={}", e);
                    } else {
                        error!(
                            "read from stream error, remote={:?}, err={}",
                            src.peer_addr(),
                            e
                        );
                        break;
                    }
                }
            }
        }

        debug!(
            "will close tcp stream, {:?} -> {:?}, need_clear={}",
            src.peer_addr(),
            dest.peer_addr(),
            need_clear
        );

        if need_clear {
            if let Err(e) = src.shutdown(Shutdown::Both) {
                error!(
                    "shutdown src stream for read error, remote={:?}, err={}",
                    src.peer_addr(),
                    e
                );
            }

            if let Err(e) = dest.shutdown(Shutdown::Both) {
                error!(
                    "shutdown dst stream for write error, remote={:?}, err={}",
                    src.peer_addr(),
                    e
                );
            }
        }
    }
}

pub struct TcpUpStreamForBdt {
    address: (String, u32),
}

impl TcpUpStreamForBdt {
    pub fn new(address: &(String, u32)) -> TcpUpStreamForBdt {
        TcpUpStreamForBdt {
            address: (address.0.clone(), address.1),
        }
    }

    pub async fn bind(&self, stream: BdtStream) -> Result<(), BuckyError> {
        let str = format!("{}:{}", self.address.0, self.address.1);

        let ret = TcpStream::connect(str).await;
        if let Err(e) = ret {
            error!(
                "connect tcp up stream error, addr={:?}, e={}",
                self.address, e
            );
            return Err(BuckyError::from(e));
        } else {
            debug!("connect tcp up stream success! addr={:?}", self.address);
        }

        // 目前只能connect发起后才能拿到port进行关联，但如果对方收到连接后立刻查询，可能还没走到这里
        // 对方要在连接上收到数据后再进行反查操作
        let up_stream = ret.unwrap();
        TcpUpStream::init_up_stream(&up_stream);

        // 保存peerid和upstream端口关联
        let port = up_stream.local_addr().unwrap().port();
        let device_id = stream.remote().0.clone();

        PEER_ASSOC_MANAGER
            .lock()
            .unwrap()
            .add(AssociationProtocol::Tcp, port, device_id);


        let up_stream2 = up_stream.clone();
        let stream2 = stream.clone();

        let t1 = task::spawn(async move {
            Self::bind_stream_up(stream, up_stream).await;
        });

        let t2 = task::spawn(async move {
            Self::bind_stream_down(up_stream2, stream2).await;
        });

        join!(t1, t2);

        // 解除关联
        PEER_ASSOC_MANAGER
            .lock()
            .unwrap()
            .remove(AssociationProtocol::Tcp, port);

        Ok(())
    }

    // BdtStream读 -> TcpStream写
    async fn bind_stream_up(mut src: BdtStream, mut dest: TcpStream) {
        let mut recv_buf = [0x00_u8; 1024 * 64];
        let mut need_clear = true;

        loop {
            let ret = async_std::io::timeout(Duration::from_secs(5), src.read(&mut recv_buf)).await;
            // let ret = src.read(&mut recv_buf).await;
            match ret {
                Ok(recv_size) => {
                    if recv_size > 0 {
                        if let Err(e) = dest.write_all(&recv_buf[0..recv_size]).await {
                            error!(
                                "write to stream error, remote={:?}, err={}",
                                dest.peer_addr(),
                                e
                            );
                            break;
                        }
                    } else {
                        need_clear = false;

                        // FIXME bdt关闭了读后，先同时关闭写
                        if let Err(e) = src.shutdown(Shutdown::Both) {
                            error!("shutdown bdt stream error: {:?} {}", src.remote(), e);
                        }

                        // src关闭了写入
                        let _r = dest.shutdown(Shutdown::Write);
                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == async_std::io::ErrorKind::TimedOut {
                        //  debug!("read timeout, err={}", e);
                    } else {
                        error!(
                            "read from bdt stream error, remote={:?}, err={}",
                            src.remote(),
                            e
                        );
                        break;
                    }
                }
            }
        }

        debug!(
            "bdt_stream {:?} -> tcp_stream {:?} end",
            src.remote(),
            dest.local_addr()
        );

        if need_clear {
            if let Err(e) = src.shutdown(Shutdown::Both) {
                error!("shutdown bdt stream error: {:?} {}", src.remote(), e);
            }

            if let Err(e) = dest.shutdown(Shutdown::Both) {
                error!(
                    "shutdown dst stream for write error, remote={:?}, err={}",
                    src.remote(),
                    e
                );
            }
        }
    }

    // TcpStream读 -> BdtStream写
    async fn bind_stream_down(mut src: TcpStream, mut dest: BdtStream) {
        let mut recv_buf = [0x00_u8; 1024 * 64];
        let mut need_clear = true;
        loop {
            let ret = async_std::io::timeout(Duration::from_secs(5), src.read(&mut recv_buf)).await;
            //let ret = src.read(&mut recv_buf).await;
            match ret {
                Ok(recv_size) => {
                    if recv_size > 0 {
                        if let Err(e) = dest.write_all(&recv_buf[0..recv_size]).await {
                            error!(
                                "write to stream error, remote={:?}, err={}",
                                dest.remote(),
                                e
                            );
                            break;
                        }
                    } else {
                        if let Err(e) = src.shutdown(Shutdown::Read) {
                            error!(
                                "shutdown src tcp stream error! remote={:?}, err={}",
                                src.peer_addr(),
                                e
                            );
                        }

                        if let Err(e) = dest.shutdown(Shutdown::Write) {
                            error!("shutdown bdt stream error: {:?} {}", dest.remote(), e);
                        }

                        need_clear = false;

                        break;
                    }
                }
                Err(e) => {
                    if e.kind() == async_std::io::ErrorKind::TimedOut {
                        //  debug!("read timeout, err={}", e);
                    } else {
                        error!(
                            "read from stream error, remote={:?}, err={}",
                            src.peer_addr(),
                            e
                        );
                        break;
                    }
                }
            }
        }

        debug!(
            "tcp_stream {:?} -> bdt_stream {:?} end",
            src.local_addr(),
            dest.remote()
        );

        if need_clear {
            if let Err(e) = src.shutdown(Shutdown::Both) {
                error!(
                    "close src tcp stream error, remote={:?}, err={}",
                    src.peer_addr(),
                    e
                );
            }

            if let Err(e) = dest.shutdown(Shutdown::Both) {
                error!("shutdown bdt stream error: {:?} {}", dest.remote(), e);
            }
        }
    }
}

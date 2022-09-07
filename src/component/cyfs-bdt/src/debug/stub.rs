use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, Shutdown},
    path::Path, 
    str::FromStr, 
    time::{Duration, Instant}
};
use async_std::{
    sync::Arc, 
    task, 
    net::{TcpListener, TcpStream},
    stream::StreamExt,
    future, 
    io::prelude::*,
    fs::File, 
};

use cyfs_base::*;
use crate::{
    stack::{WeakStack, Stack}, 
    tunnel::{BuildTunnelParams}, 
    datagram::{self, DatagramOptions},
    download::*,
    DownloadTaskControl, 
    TaskControlState,
    types::*,
    SingleDownloadContext
};
use super::command::*;
use super::super::sn::client::SnStatus;

struct DebugStubImpl {
    stack: WeakStack, 
    listener: TcpListener,
}

#[derive(Clone)]
pub struct Config {
    pub local: String,
    pub port: u16
}

#[derive(Clone)]
pub struct DebugStub(Arc<DebugStubImpl>);

impl DebugStub {
    pub async fn open(weak_stack: WeakStack) -> BuckyResult<Self> {
        let stack = Stack::from(&weak_stack);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from_str(stack.config().debug.as_ref().unwrap().local.as_str()).unwrap()), 
            stack.config().debug.as_ref().unwrap().port);
        let listener = TcpListener::bind(addr).await?;
        Ok(Self(Arc::new(DebugStubImpl {
            stack: weak_stack, 
            listener
        })))
    } 

    pub fn listen(&self) {
        const READ_CMD_TIMEOUT: u64 = 30;

        let stub = self.clone();
        task::spawn(async move {
            let mut incoming = stub.0.listener.incoming();
            loop {
                if let Some(stream) = incoming.next().await {
                    if let Ok(stream) = stream {
                        let stub = stub.clone();
                        task::spawn(async move {
                            let mut stream = stream;
                            let mut command = String::new();
                            match future::timeout(Duration::from_secs(READ_CMD_TIMEOUT), stream.read_to_string(&mut command)).await {
                                Err(_) => {
                                    error!("read cmd timeout!");
                                },
                                Ok(_) => {
                                    stub.handle_command(command, stream).await;
                                }
                            }
                        });
                    }
                }
            }
        });

        let stack = Stack::from(&self.0.stack);
        let debug_tunnel = stack.datagram_manager().bind_reserved(datagram::ReservedVPort::Debug).unwrap();
        task::spawn(async move {
            loop {
                match debug_tunnel.recv_v().await {
                    Ok(datagrams) => {
                        for datagram in datagrams {
                            //let resp = b"debug";
                            let mut options = datagram.options.clone();
                            let _ = debug_tunnel.send_to(
                                datagram.data.as_ref(),//resp.as_ref(), 
                                &mut options, 
                                &datagram.source.remote, 
                                datagram.source.vport);
                        }
                    }, 
                    Err(_err) => {
                        
                    }
                }
            }
        });
    }

    async fn handle_command(&self, command: String, tunnel: TcpStream) {
        println!("command:{}", command);
        let stack = Stack::from(&self.0.stack);
        let mut tunnel = tunnel;
        match DebugCommand::from_str(&stack, &command).await {
            Ok(command) => {
                if let Err(err) = match command {
                    DebugCommand::Test(_) => self.test(tunnel.clone()).await, 
                    DebugCommand::Ping(command) => self.ping(tunnel.clone(), command).await, 
                    DebugCommand::Nc(command) => self.nc(tunnel.clone(), command).await,
                    DebugCommand::GetChunk(command) => self.get_chunk(tunnel.clone(), command).await,
                    DebugCommand::GetFile(command) => self.get_file(tunnel.clone(), command).await,
                    DebugCommand::PutChunk(command) => self.put_chunk(tunnel.clone(), command).await,
                    DebugCommand::PutFile(command) => self.put_file(tunnel.clone(), command).await,
                    DebugCommand::SnConnStatus(command) => self.sn_conn_status(tunnel.clone(), command).await,
                    DebugCommand::BenchDatagram(command) => self.bench_datagram(tunnel.clone(), command).await,
                } {
                    let _ = tunnel.write_all(err.as_ref()).await;
                }
            }, 
            Err(err) => {
                let _ = tunnel.write_all(err.as_ref()).await;
            }
        }
    }

    async fn test(&self, tunnel: TcpStream) -> Result<(), String> {
        let mut tunnel = tunnel;

        let _ = tunnel.write_all(format!("hello\r\n").as_bytes()).await;
        let _ = tunnel.write_all(format!("bdt\r\n").as_bytes()).await;

        Ok(())
    }

    async fn sn_conn_status(&self, tunnel: TcpStream, command: DebugCommandSnConnStatus) -> Result<(), String> {
        let mut tunnel = tunnel;

        let stack = Stack::from(&self.0.stack);
        let dev_id_option = {
            if command.sn.is_none() {
                let sn_list = stack.sn_client().sn_list();
                let sn = sn_list.get(0);
                match sn {
                    Some(sn) => {
                        let sn = sn.clone();
                        Some(sn)
                    },
                    _ => None,
                }
            } else {
                command.sn
            }
        };
        if dev_id_option.is_none() {
            let _ = tunnel.write_all("Err: sn is none\r\n".as_ref()).await;
            return Ok(())
        }

        let sn_dev_id = dev_id_option.unwrap();
        let timeout = {
            if command.timeout_sec == 0 {
                6
            } else {
                command.timeout_sec
            }
        };

        let sleep_ms = 200; 
        let mut counter = timeout*(1000/sleep_ms);
        loop {
            let sn_status = stack.sn_client().status_of(&sn_dev_id);

            match sn_status {
                Some(st) => {
                    if st == SnStatus::Online {
                        let _ = tunnel.write_all("Ok: sn connected\r\n".as_ref()).await;

                        return Ok(())
                    }
                },
                _ => {}
            }

            counter -= 1;
            if counter == 0 {
                break ;
            }

            task::sleep(Duration::from_millis(sleep_ms)).await;
        }

        let _ = tunnel.write_all("Err: sn connect timeout\r\n".as_ref()).await;

        Ok(())
    }

    async fn ping(&self, tunnel: TcpStream, command: DebugCommandPing) -> Result<(), String> {
        let mut tunnel = tunnel;
        let stack = Stack::from(&self.0.stack);
        let datagram = stack.datagram_manager().bind(0)
            .map_err(|err| format!("deamon bind datagram tunnel failed for {}\r\n", err))?;
        for _ in 0..command.count {
            let mut options = DatagramOptions::default();
            let _ = tunnel.write_all("send ping.\r\n".as_ref()).await;

            let ts = cyfs_base::bucky_time_now();
            options.sequence = Some(TempSeq::from(ts as u32));
            let _ = datagram.send_to(
                "debug".as_ref(), 
                &mut options, 
                &command.remote.desc().device_id(), 
                datagram::ReservedVPort::Debug.into());
            match future::timeout(command.timeout, datagram.recv_v()).await {
                Err(_err) => {
                    let _ = tunnel.write_all("timeout\r\n".as_ref()).await;
                },
                Ok(res) => {
                    let datagrams = res.unwrap();
                    for datagram in datagrams {
                        if let Some(opt) = datagram.options.sequence {
                            if opt == options.sequence.unwrap() {
                                let s = format!("respose. time: {:.1} ms\r\n", 
                                                    (cyfs_base::bucky_time_now() - ts) as f64 / 1000.0);
                                let _ = tunnel.write_all(s.as_bytes()).await;
                                break ;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn bench_datagram(&self, tunnel: TcpStream, command: DebugCommandBenchDatagram) -> Result<(), String> {
        let mut tunnel = tunnel;

        let from = 1;
        let to = 65535;
        let plaintext = command.plaintext;

        let s = format!("bench_datagram: plaintext:{} timeout:{:?} from:{} to:{}\r\n",
            plaintext, command.timeout, from, to);
        let _ = tunnel.write_all(s.as_bytes()).await;

        let mut n_ok = 0;
        let stack = Stack::from(&self.0.stack);
        let datagram = stack.datagram_manager().bind(0)
            .map_err(|err| format!("deamon bind datagram tunnel failed for {}\r\n", err))?;
        for i in from..to {
            let mut options = DatagramOptions::default();
            let _ = tunnel.write_all("send data.\r\n".as_ref()).await;

            let data = rand_data_gen(i);
            let ts = cyfs_base::bucky_time_now();
            options.sequence = Some(TempSeq::from(ts as u32));
            if i%2 == 0 {
                options.create_time = Some(ts+10);
            }
            if i%4 == 0 {
                options.send_time = Some(ts+20);
            }
            if i%8 == 0 {
                options.author_id = Some(command.remote.desc().device_id().clone());
            }
            options.plaintext = plaintext;
            let _ = datagram.send_to(
                &data, 
                &mut options, 
                &command.remote.desc().device_id(), 
                datagram::ReservedVPort::Debug.into());
            match future::timeout(command.timeout, datagram.recv_v()).await {
                Err(_err) => {
                    let _ = tunnel.write_all("timeout\r\n".as_ref()).await;
                },
                Ok(res) => {
                    let datagrams = res.unwrap();
                    for datagram in datagrams {
                        if let Some(opt) = datagram.options.sequence {
                            if opt == options.sequence.unwrap() {
                                let md5_recv = md5::compute(&datagram.data);
                                let md5_send = md5::compute(&data);
                                if md5_recv == md5_send {
                                    n_ok += 1;
                                }
                                let s = format!("respose: plaintext: {} time: {:.1} ms, success {}/{}, fail {}\r\n", 
                                                    options.plaintext,
                                                    (cyfs_base::bucky_time_now() - ts) as f64 / 1000.0,
                                                    n_ok,
                                                    i,
                                                    i-n_ok);
                                let _ = tunnel.write_all(s.as_bytes()).await;
                                break ;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn nc(&self, tunnel: TcpStream, command: DebugCommandNc) -> Result<(), String> {
        let mut tunnel = tunnel;
        let stack = Stack::from(&self.0.stack);
        let _ = tunnel.write_all("connecting stream\r\n".as_ref()).await;

        let question = b"question?";
        let mut conn = stack.stream_manager().connect(
            command.port, 
            question.to_vec(), 
            BuildTunnelParams {
                remote_const: command.remote.desc().clone(), 
                remote_sn: vec![], 
                remote_desc: Some(command.remote.clone())
        }).await.map_err(|err| format!("Err: {}\r\n", err.msg().to_string()))?;

        let _ = tunnel.write_all("Connect success, read answer\r\n".as_ref()).await;

        let mut answer = [0; 128];
        match conn.read(&mut answer).await {
            Ok(len) => {
                let s = format!("Read answer success, len={} content={:?}\r\n", 
                    len, String::from_utf8(answer[..len].to_vec()).expect(""));
                let _ = tunnel.write_all(s.as_bytes()).await;
            },
            Err(e) => {
                let s = format!("Read answer fail, err={}\r\n", e);
                let _ = tunnel.write_all(s.as_bytes()).await;
                return Ok(());
            }
        }

        let _ = conn.write_all(b"hello world.").await;

        let mut buf = [0u8; 128];
        match conn.read(&mut buf).await {
            Ok(len) => {
                let s = format!("Read data success, len={} content={:?}\r\n", 
                    len, String::from_utf8(buf[..len].to_vec()).expect(""));
                let _ = tunnel.write_all(s.as_bytes()).await;
            },
            Err(e) => {
                let s = format!("Read data fail, err={}\r\n", e);
                let _ = tunnel.write_all(s.as_bytes()).await;
                return Ok(());
            }
        }

        let _ = tunnel.write_all("Ok: stream connected\r\n".as_ref()).await;

        let _ = conn.shutdown(Shutdown::Both);

        Ok(())
    }

    async fn get_chunk(&self, tunnel: TcpStream, command: DebugCommandGetChunk) -> Result<(), String> {
        let mut tunnel = tunnel;
        let stack = Stack::from(&self.0.stack);
        let chunk_id = command.chunk_id;
        let remotes = command.remotes;
        let timeout = command.timeout;
        let local_path = command.local_path;

        let _ = tunnel.write_all("start downloading chunk..\r\n".as_ref()).await;
        let task = download_chunk_to_path(&stack,
            chunk_id,
            SingleDownloadContext::streams(None, remotes),
            &local_path).await
            .map_err(|e| format!("download err: {}\r\n", e))?;

        let _ = tunnel.write_all("waiting..\r\n".as_ref()).await;
        let task_start_time = Instant::now();
        let ret = watchdog_download_finished(task, timeout).await;
        if ret.is_ok() {
            let size = get_filesize(&local_path);
            let cost = Instant::now() - task_start_time;
            let cost_sec = (cost.as_millis() as f64) / 1000.0;
            let speed = (size as f64) * 8.0 / cost_sec / 1000000.0;
            let _ = tunnel.write_all(format!("download chunk finish.\r\nsize: {:.1} MB\r\ncost: {:.1} s\r\nspeed: {:.1} Mbps\r\n", 
            size/1024/1024, cost_sec, speed).as_bytes()).await;
        }
        ret
    }

    async fn get_file(&self, tunnel: TcpStream, command: DebugCommandGetFile) -> Result<(), String> {
        let mut tunnel = tunnel;
        let stack = Stack::from(&self.0.stack);
        let file_id = command.file_id;
        let remotes = command.remotes;
        let timeout = command.timeout;
        let local_path = command.local_path;

        let _ = tunnel.write_all("start downloading file..\r\n".as_ref()).await;
        let task = download_file_to_path(&stack, file_id, 
            SingleDownloadContext::streams(None, remotes),
            &local_path).await.map_err(|e| {
                format!("download err: {}\r\n", e)
        })?;

        let _ = tunnel.write_all("waitting..\r\n".as_ref()).await;
        let ret = watchdog_download_finished(task, timeout).await;
        if ret.is_ok() {
            let _ = tunnel.write_all("download file finish.\r\n".as_ref()).await;
        }
        ret
     }

     async fn put_chunk(&self, tunnel: TcpStream, command: DebugCommandPutChunk) -> Result<(), String> {
        let mut tunnel = tunnel;
        let stack = Stack::from(&self.0.stack);
        let local_path = command.local_path;

        if local_path.as_path().exists() {
            let mut file = async_std::fs::File::open(local_path.as_path()).await.map_err(|e| {
                format!("open file err: {}\r\n", e)
            })?;
            let mut content = Vec::<u8>::new();
            let _ = file.read_to_end(&mut content).await.map_err(|e| {
                format!("read file err: {}\r\n", e)
            })?;

            if content.len() == 0 {
                return Err(format!("file size is zero\r\n"));
            }

            match ChunkId::calculate(content.as_slice()).await {
                Ok(chunk_id) => {
                    let _ = track_chunk_in_path(&stack, &chunk_id, local_path).await
                        .map_err(|e|  format!("put chunk err: {}\r\n", e))?;
                    let _ = tunnel.write_all(format!("put chunk success. chunk_id: {}\r\n", 
                    chunk_id.to_string()).as_bytes()).await;
                    Ok(())
                }, 
                Err(e) => {
                    Err(format!("calculate chunk id err: {}\r\n", e))
                }
            }
        } else {
            Err(format!("file not exists: {}\r\n", local_path.to_str().unwrap()))
        }
     }

     async fn put_file(&self, tunnel: TcpStream, command: DebugCommandPutFile) -> Result<(), String> {
        let mut tunnel = tunnel;
        let stack = Stack::from(&self.0.stack);
        let local_path = command.local_path;

        if local_path.as_path().exists() {
            let chunkids = {
                let _ = tunnel.write_all("calculate chunkid by file..\r\n".as_ref()).await;

                let chunk_size: usize = 10 * 1024 * 1024;
                let mut chunkids = Vec::new();
                let mut file = File::open(local_path.as_path()).await.map_err(|e| {
                    format!("open file err: {}\r\n", e)
                })?;
                loop {
                    let mut buf = vec![0u8; chunk_size];
                    let len = file.read(&mut buf).await.map_err(|e| {
                        format!("read file err: {}\r\n", e)
                    })?;
                    if len > 0 {
                        if len < chunk_size {
                            buf.truncate(len);
                        }
                        let hash = hash_data(&buf[..]);
                        let chunkid = ChunkId::new(&hash, buf.len() as u32);
                        chunkids.push(chunkid);
                    }
                    if len < chunk_size {
                        break ;
                    }
                }
                chunkids
            };

            let (hash, len) = hash_file(local_path.as_path()).await.map_err(|e| {
                format!("hash file err: {}\r\n", e)
            })?;
            let file = cyfs_base::File::new(
                ObjectId::default(),
                len,
                hash,
                ChunkList::ChunkInList(chunkids)
            ).no_create_time().build();

            let buf_len_ret = file.raw_measure(&None);
            if buf_len_ret.is_err() {
                return Err(format!("raw_measure err\r\n"));
            }
            let mut buf = vec![0u8; buf_len_ret.unwrap()];
            let encode_ret = file.raw_encode(buf.as_mut_slice(), &None);
            if encode_ret.is_err() {
                return Err(format!("raw_encode err\r\n"));
            }

            track_file_in_path(&stack, file, local_path).await
                .map_err(|e| format!("track file err {}\r\n", e))?;

            let _ = tunnel.write_all(format!("put file sucess. file_id: {}\r\n", 
                hex::encode(buf)).as_bytes()).await;

            Ok(())
        } else {
            Err(format!("{} not exists\r\n", &local_path.to_str().unwrap()))
        }
    }
}

async fn watchdog_download_finished(task: Box<dyn DownloadTaskControl>, timeout: u32) -> Result<(), String> {
    let mut _timeout = 1800; //todo: when bdt support download speed, use timeout instead
    let mut i = 0;

    loop {
        match task.control_state() {
            TaskControlState::Finished(_) => {
                break Ok(());
            },
            TaskControlState::Downloading(speed, _) => {
                if speed > 0 {
                    i = 0;

                    if _timeout == 1800 { //todo
                        _timeout = timeout;
                    }
                } else {
                    i += 1;
                }
            },
            TaskControlState::Canceled => {
                break Err(format!("download canceled\r\n"));
            },
            TaskControlState::Paused => {
            },
            TaskControlState::Err(e) => {
                break Err(format!("download err, code: {:?}\r\n", e));
            },
        }

        if i >= _timeout {
            let _ = task.cancel();
            break Err(format!("download timeout\r\n"));
        }

        task::sleep(Duration::from_secs(1)).await;
    }
}

fn get_filesize(path: &Path) -> u64 {
    let res = std::fs::File::open(path);
    if res.is_ok() {
        let file = res.unwrap();
        return file.metadata().unwrap().len();
    }

    0
}

fn rand_data_gen(len: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.resize(len, 0u8);

    let mut r = 0;
    for i in 0..len {
        if i%10 == 0 {
            r = rand::random::<u8>();
        }
        buf[i] = r;
    }

    buf
}

fn rand_data_gen_buf(len: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.resize(len + 8, 0u8);

    buf[0..8].copy_from_slice(&len.to_be_bytes());

    let mut r = 0;
    for i in 8..len {
        if i%10 == 0 {
            r = rand::random::<u8>();
        }
        buf[i] = r;
    }

    buf
}
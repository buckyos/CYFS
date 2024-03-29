use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr, Shutdown},
    path::Path, 
    str::FromStr, 
    time::{Duration, Instant},
};
use async_std::{
    sync::Arc, 
    task, 
    net::{TcpListener, TcpStream},
    stream::StreamExt,
    future, 
    io::prelude::*,
    // fs::File, 
};

use cyfs_base::*;
use crate::{
    stack::{WeakStack, Stack}, 
    tunnel::{BuildTunnelParams}, 
    datagram::{self, DatagramOptions}, 
    types::*,
    ndn::*, 
    utils::*,
};
use super::command::*;
use super::super::sn::client::SnStatus;

struct DebugStubImpl {
    stack: WeakStack, 
    listener: TcpListener,
    chunk_store: MemChunkStore, 
}

#[derive(Clone)]
pub struct Config {
    pub local: String,
    pub port: u16,
    pub chunk_store: MemChunkStore,
}

#[derive(Clone)]
pub struct DebugStub(Arc<DebugStubImpl>);

impl DebugStub {
    pub async fn open(weak_stack: WeakStack, chunk_store: MemChunkStore) -> BuckyResult<Self> {
        let stack = Stack::from(&weak_stack);
        let addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::from_str(stack.config().debug.as_ref().unwrap().local.as_str()).unwrap()), 
            stack.config().debug.as_ref().unwrap().port);
        let listener = TcpListener::bind(addr).await?;
        Ok(Self(Arc::new(DebugStubImpl {
            stack: weak_stack, 
            listener,
            chunk_store,
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
            let sn_status = stack.sn_client().ping().status();

            if let Some(sn_status) = sn_status {
                if let SnStatus::Online = sn_status {
                    let _ = tunnel.write_all("Ok: sn connected\r\n".as_ref()).await;

                    return Ok(())
                }
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
        let stack = Stack::from(&self.0.stack);
        let task_num = if command.task_num == 0 {
            1
        } else {
            command.task_num
        };
        let mut tasks = vec![];

        for task_id in 0..task_num {
            let mut t = tunnel.clone();
            let c = command.clone();
            let s = stack.clone();
            tasks.push(task::spawn(async move {
                match nc_task(t.clone(), c, s, task_id).await {
                    Err(e) => {
                        let _ = t.write_all(format!("nc_task err={}\r\n", e).as_ref()).await;
                    },
                    Ok(_) => {
                    }
                }
            }));
        }

        for t in tasks {
            let _ = t.await;
        }

        Ok(())
    }

    async fn get_chunk(&self, tunnel: TcpStream, command: DebugCommandGetChunk) -> Result<(), String> {
        let mut tunnel = tunnel;

        let chunk_id = command.chunk_id;
        let remotes = command.remotes;
        //let local_path = command.local_path;

        let stack = Stack::from(&self.0.stack);

        let chunk_store = self.0.chunk_store.clone();
        let context = SampleDownloadContext::desc_streams("".to_string(), remotes);
        let begin = Instant::now();
        match download_chunk(&stack, chunk_id.clone(),None, context).await {
            Ok((_, reader)) => {
                chunk_store.write_chunk(&chunk_id, reader).await.unwrap();
                match future::timeout(Duration::from_secs(600), get_chunk_wait_finish(stack.clone(), chunk_id.clone())).await {
                    Err(e) => {
                        let _ = tunnel.write_all(format!("get_chunk_wait_finish err={}\r\n", e).as_ref()).await;
                    },
                    Ok(r) => {
                        match r {
                            Ok(n) => {
                                let cost_secs = begin.elapsed().as_secs_f64();
                                let _ = tunnel.write_all(format!("get success\r\n").as_ref()).await;
                                if chunk_id.len() != n {
                                    let _ = tunnel.write_all(format!("data wrong, recv_len={} want={}\r\n", n, chunk_id.len()).as_ref()).await;
                                } else {
                                    let len = n as f64;
                                    let speed = if cost_secs > 0.0 {
                                        len / cost_secs / 1024.0
                                    } else {
                                        999999.9
                                    };
                                    let _ = tunnel.write_all(format!("cost={:.3}s len={:.1}KB speed={:.1}KB/s\r\n",
                                    cost_secs, len/1024.0, speed).as_ref()).await;
                                }
                            },
                            Err(e) => {
                                let _ = tunnel.write_all(format!("get_chunk_wait_finish err={}\r\n", e).as_ref()).await;
                            }
                        }
                    }
                }
            },
            Err(e) => {
                let _ = tunnel.write_all(format!("download_chunk err={}\r\n", e).as_ref()).await;
            }
        }

        Ok(())
    }

    async fn get_file(&self, tunnel: TcpStream, command: DebugCommandGetFile) -> Result<(), String> {
        let mut tunnel = tunnel;

        let file_id = command.file_id;
        let remotes = command.remotes;
        let timeout = command.timeout;
        let local_path = command.local_path;


        let _ = tunnel.write_all("start downloading file..\r\n".as_ref()).await;

        let stack = Stack::from(&self.0.stack);
        let context = SampleDownloadContext::id_streams(&stack, "".to_owned(), &remotes).await
            .map_err(|e| format!("download err: {}\r\n", e))?;
        let (_, reader) = download_file(
            &stack, 
            file_id.clone(), 
            None,  
            context)
            .await.map_err(|e| {
                format!("download err: {}\r\n", e)
            })?;
        
        let _ = future::timeout(Duration::from_secs(timeout as u64), tunnel.write_all("waitting..\r\n".as_ref())).await;

        LocalChunkListWriter::from_file(local_path, &file_id)
            .map_err(|e| {
                format!("download err: {}\r\n", e)
            })?
            .write(reader).await
            .map_err(|e| {
                format!("download err: {}\r\n", e)
            })?;

        let _ = tunnel.write_all("download file finish.\r\n".as_ref()).await;
        Ok(())
     }

     async fn put_chunk(&self, tunnel: TcpStream, command: DebugCommandPutChunk) -> Result<(), String> {
        let mut tunnel = tunnel;
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

            let chunk_store = self.0.chunk_store.clone();
            match ChunkId::calculate(content.as_slice()).await {
                Ok(chunk_id) => {
                    match chunk_store.add(chunk_id.clone(), Arc::new(content)).await {
                        Ok(_) => {
                            let _ = tunnel.write_all(format!("put chunk success, chunk_id={}\r\n", chunk_id).as_ref()).await;
                        },
                        Err(e) => {
                            let _ = tunnel.write_all(format!("put chunk fail, err={}\r\n", e).as_ref()).await;
                        }
                    }
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

     async fn put_file(&self, _tunnel: TcpStream, _command: DebugCommandPutFile) -> Result<(), String> {
        // let mut tunnel = tunnel;
        // let stack = Stack::from(&self.0.stack);
        // let local_path = command.local_path;

        // if local_path.as_path().exists() {
        //     let chunkids = {
        //         let _ = tunnel.write_all("calculate chunkid by file..\r\n".as_ref()).await;

        //         let chunk_size: usize = 10 * 1024 * 1024;
        //         let mut chunkids = Vec::new();
        //         let mut file = File::open(local_path.as_path()).await.map_err(|e| {
        //             format!("open file err: {}\r\n", e)
        //         })?;
        //         loop {
        //             let mut buf = vec![0u8; chunk_size];
        //             let len = file.read(&mut buf).await.map_err(|e| {
        //                 format!("read file err: {}\r\n", e)
        //             })?;
        //             if len > 0 {
        //                 if len < chunk_size {
        //                     buf.truncate(len);
        //                 }
        //                 let hash = hash_data(&buf[..]);
        //                 let chunkid = ChunkId::new(&hash, buf.len() as u32);
        //                 chunkids.push(chunkid);
        //             }
        //             if len < chunk_size {
        //                 break ;
        //             }
        //         }
        //         chunkids
        //     };

        //     let (hash, len) = hash_file(local_path.as_path()).await.map_err(|e| {
        //         format!("hash file err: {}\r\n", e)
        //     })?;
        //     let file = cyfs_base::File::new(
        //         ObjectId::default(),
        //         len,
        //         hash,
        //         ChunkList::ChunkInList(chunkids)
        //     ).no_create_time().build();

        //     let buf_len_ret = file.raw_measure(&None);
        //     if buf_len_ret.is_err() {
        //         return Err(format!("raw_measure err\r\n"));
        //     }
        //     let mut buf = vec![0u8; buf_len_ret.unwrap()];
        //     let encode_ret = file.raw_encode(buf.as_mut_slice(), &None);
        //     if encode_ret.is_err() {
        //         return Err(format!("raw_encode err\r\n"));
        //     }

        //     track_file_in_path(&stack, file, local_path).await
        //         .map_err(|e| format!("track file err {}\r\n", e))?;

        //     let _ = tunnel.write_all(format!("put file sucess. file_id: {}\r\n", 
        //         hex::encode(buf)).as_bytes()).await;

        //     Ok(())
        // } else {
        //     Err(format!("{} not exists\r\n", &local_path.to_str().unwrap()))
        // }
        Err("not supported now".to_owned())
    }
}

async fn watchdog_download_finished(task: Box<dyn DownloadTask>, timeout: u32) -> Result<(), String> {
    let mut _timeout = 1800; //todo: when bdt support download speed, use timeout instead
    let mut i = 0;

    loop {
        match task.state() {
            NdnTaskState::Finished => {
                break Ok(());
            },
            NdnTaskState::Running => {
                if task.cur_speed() > 0 {
                    i = 0;

                    if _timeout == 1800 { //todo
                        _timeout = timeout;
                    }
                } else {
                    i += 1;
                }
            },
            NdnTaskState::Error(e) => {
                break Err(format!("download err, code: {:?}\r\n", e));
            },
            _ => {

            }
        }

        if i >= _timeout {
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

fn rand_char(len: usize) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.resize(len, 0u8);

    for i in 0..len {
        buf[i] = 97 + rand::random::<u8>() % 26;
    }

    buf
}

async fn get_chunk_wait_finish(stack: Stack, chunk_id: ChunkId) -> BuckyResult<usize> {
    let mut len = 0;
    loop {
        let ret = stack.ndn().chunk_manager().store().get(&chunk_id).await;
        if let Ok(mut reader) = ret {
            let mut content = vec![0u8; 2048];

            loop {
                let n = reader.read(content.as_mut_slice()).await?;
                if n == 0 {
                    break ;
                }
                len += n;
            }

            return Ok(len);
        } else {
            task::sleep(Duration::from_millis(200)).await;
        }
    }
}

async fn nc_task(tunnel: TcpStream, command: DebugCommandNc, stack: Stack, task_id: u32) -> Result<(), String> {
    let mut tunnel = tunnel;
    let _ = tunnel.write_all(format!("[{}] connecting stream\r\n", task_id).as_ref()).await;

    let question = b"question?";
    let mut conn = stack.stream_manager().connect(
        command.port, 
        question.to_vec(), 
        BuildTunnelParams {
            remote_const: command.remote.desc().clone(), 
            remote_sn: None, 
            remote_desc: Some(command.remote.clone())
    }).await.map_err(|err| format!("Err: {}\r\n", err.msg().to_string()))?;

    let _ = tunnel.write_all(format!("[{}] Connect success, read answer\r\n", task_id).as_ref()).await;

    let mut answer = [0; 128];
    match conn.read(&mut answer).await {
        Ok(len) => {
            let s = format!("[{}] Read answer success, len={} content={:?}\r\n", 
                task_id, len, String::from_utf8(answer[..len].to_vec()).expect(""));
            let _ = tunnel.write_all(s.as_bytes()).await;
        },
        Err(e) => {
            let s = format!("[{}] Read answer fail, err={}\r\n", task_id, e);
            let _ = tunnel.write_all(s.as_bytes()).await;
            return Ok(());
        }
    }

    let _ = conn.write_all(b"hello world").await;

    let mut buf = [0u8; 128];
    match conn.read(&mut buf).await {
        Ok(len) => {
            let s = format!("[{}] Read data success, len={} content={:?}\r\n", 
                task_id, len, String::from_utf8(buf[..len].to_vec()).expect(""));
            let _ = tunnel.write_all(s.as_bytes()).await;
        },
        Err(e) => {
            let s = format!("[{}] Read data fail, err={}\r\n", task_id, e);
            let _ = tunnel.write_all(s.as_bytes()).await;
            return Ok(());
        }
    }

    let _ = tunnel.write_all(format!("[{}] Ok: stream connected\r\n", task_id).as_ref()).await;

    if command.bench > 0 {
        let _ = tunnel.write_all(format!("[{}] start bench size={}MB\r\n", task_id, command.bench).as_ref()).await;

        let buf = rand_char(1024);
        let mut i: u32 = 0;
        let max = command.bench * 1024;
        let begin = Instant::now();
        loop {
            match conn.write_all(&buf).await {
                Ok(_) => {
                    i += 1;
                },
                Err(e) => {
                    let _ = tunnel.write_all(format!("[{}] write err={}\r\n", task_id, e).as_ref()).await;
                    break;
                }
            }
            if i >= max {
                break;
            }
        }
        let cost = begin.elapsed().as_secs_f64();
         let speed = if cost > 0.0 {
            i as f64 / cost
        } else {
            999999.9
        };
        let _ = tunnel.write_all(format!("[{}] bench over. cost={:.3}s len={}KB speed={:.1}KB/s\r\n",
           task_id, cost, i, speed).as_ref()).await;
    }

    let _ = conn.shutdown(Shutdown::Both);

    Ok(())
}
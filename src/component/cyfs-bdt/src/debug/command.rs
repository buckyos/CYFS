use std::{
    str::FromStr, 
    path::{ Path, PathBuf },
    time::Duration,
};
use clap::{App, SubCommand, Arg};
use cyfs_base::*;
use crate::stack::Stack;

pub fn debug_command_line() -> clap::App<'static, 'static> {
    App::new("bdt-debug").about("bdt stack debug")
        .arg(Arg::with_name("host").short("h").long("host").value_name("host").help("connect remote host").default_value("127.0.0.1"))
        .arg(Arg::with_name("port").short("p").long("port").value_name("port").help("local server port").default_value("12345"))
        .subcommand(SubCommand::with_name("test"))
        .subcommand(SubCommand::with_name("ping")
            .arg(Arg::with_name("remote").required(true))
            .arg(Arg::with_name("count").required(true))
            .arg(Arg::with_name("timeout").required(true))
        )
        .subcommand(SubCommand::with_name("nc")
            .arg(Arg::with_name("remote").required(true))
            .arg(Arg::with_name("port").required(true))
            .arg(Arg::with_name("bench").required(false))
            .arg(Arg::with_name("task").required(false))
        )
        .subcommand(SubCommand::with_name("get_chunk")
            .arg(Arg::with_name("remotes").required(true))
            .arg(Arg::with_name("timeout").required(true))
            .arg(Arg::with_name("chunk_id").required(true))
            .arg(Arg::with_name("local_path").required(false))
        )
        .subcommand(SubCommand::with_name("get_file")
            .arg(Arg::with_name("remotes").required(true))
            .arg(Arg::with_name("timeout").required(true))
            .arg(Arg::with_name("file_id").required(true))
            .arg(Arg::with_name("local_path").required(true))
        )
        .subcommand(SubCommand::with_name("put_chunk")
            .arg(Arg::with_name("local_path").required(true))
        )
        .subcommand(SubCommand::with_name("put_file")
            .arg(Arg::with_name("local_path").required(true))
        )
        .subcommand(SubCommand::with_name("sn_conn_status")
            .arg(Arg::with_name("timeout").required(true))
        )
        .subcommand(SubCommand::with_name("bench_datagram")
            .arg(Arg::with_name("remote").required(true))
            .arg(Arg::with_name("plaintext").required(true))
            .arg(Arg::with_name("timeout").required(true))
        )
        .subcommand(SubCommand::with_name("sn_bench_ping")
            .arg(Arg::with_name("load").required(true))
            .arg(Arg::with_name("device").required(true))
            .arg(Arg::with_name("interval").required(true))
            .arg(Arg::with_name("timeout").required(true))
        )
}

pub enum DebugCommand {
    Test(DebugCommandTest),
    Ping(DebugCommandPing), 
    Nc(DebugCommandNc), 
    GetChunk(DebugCommandGetChunk),
    GetFile(DebugCommandGetFile),
    PutChunk(DebugCommandPutChunk),
    PutFile(DebugCommandPutFile),
    SnConnStatus(DebugCommandSnConnStatus),
    BenchDatagram(DebugCommandBenchDatagram),
}

impl DebugCommand {
    pub async fn from_str(stack: &Stack, command: &str) -> Result<Self, String> {
        let params = debug_command_line().get_matches_from_safe(command.split(" "))
            .map_err(|err| err.message)?;
        let subcommand = params.subcommand_name().ok_or_else(|| "no subcommand\r\n".to_string())?;
        match subcommand {
            "test" => {
                let _ = params.subcommand_matches("test").unwrap();
                Ok(Self::Test(DebugCommandTest{}))
            },
            "ping" => {
                let subcommand = params.subcommand_matches("ping").unwrap();
                let remote = remote_device(stack, subcommand.value_of("remote").unwrap()).await
                    .map_err(|err| format!("load remote desc {} failed for {}\r\n", subcommand.value_of("remote").unwrap(), err))?;
                let count = u32::from_str(subcommand.value_of("count").unwrap()).unwrap();
                let timeout = u64::from_str(subcommand.value_of("timeout").unwrap()).unwrap();

                Ok(Self::Ping(DebugCommandPing {
                    remote, 
                    count, 
                    timeout: Duration::from_secs(timeout)
                }))
            },
            "nc" => {
                let subcommand = params.subcommand_matches("nc").unwrap();
                let remote = remote_device(stack, subcommand.value_of("remote").unwrap()).await
                    .map_err(|err| format!("load remote desc {} failed for {}\r\n", subcommand.value_of("remote").unwrap(), err))?;
                let port = u16::from_str(subcommand.value_of("port").unwrap()).unwrap();
                let bench = u32::from_str(subcommand.value_of("bench").unwrap_or("0")).unwrap();
                let task_num = u32::from_str(subcommand.value_of("task").unwrap_or("1")).unwrap();
                Ok(Self::Nc(DebugCommandNc {
                    remote, 
                    port, 
                    timeout: Duration::from_secs(8),
                    bench,
                    task_num,
                }))
            }, 
            "get_chunk" => {
                let subcommand = params.subcommand_matches("get_chunk").unwrap();
                let reomotes_str = String::from_str(subcommand.value_of("remotes").unwrap()).unwrap();
                let remotes_split = reomotes_str.split(",");
                let mut remotes = Vec::new();
                for remote_file in remotes_split {
                    let remote = remote_device(stack, remote_file).await
                    .map_err(|err| format!("load remote desc {} failed for {}\r\n", subcommand.value_of("remote").unwrap(), err))?;
                    remotes.push(remote.desc().clone());
                }
                let chunk_id_str = String::from_str(subcommand.value_of("chunk_id").unwrap()).unwrap();
                let chunk_id = ChunkId::from_str(&chunk_id_str.as_str()).map_err(|err| format!("load chunk_id {} fail: {}\r\n",
                    chunk_id_str, err))?;
                let local_path_str = String::from_str(subcommand.value_of("local_path").unwrap_or("default")).unwrap();
                let local_path = PathBuf::from_str(&local_path_str.as_str()).unwrap();
                let timeout = u32::from_str(subcommand.value_of("timeout").unwrap()).unwrap();

                Ok(Self::GetChunk(DebugCommandGetChunk {
                    remotes, 
                    timeout, 
                    chunk_id, 
                    local_path
                }))
            },
            "get_file" => {
                let subcommand = params.subcommand_matches("get_file").unwrap();
                let reomotes_str = String::from_str(subcommand.value_of("remotes").unwrap()).unwrap();
                let remotes_split = reomotes_str.split(",");
                let mut remotes = Vec::new();
                for remote_file in remotes_split {
                    let remote = remote_device(stack, remote_file).await
                    .map_err(|err| format!("load remote desc {} failed for {}\r\n", subcommand.value_of("remote").unwrap(), err))?;
                    remotes.push(remote.desc().device_id());
                }
                let file_id_str = String::from_str(subcommand.value_of("file_id").unwrap()).unwrap();
                let hex_data = hex::decode(file_id_str.as_bytes()).unwrap();
                let (file_id, _) = File::raw_decode(hex_data.as_slice()).map_err(|err| format!("load file_id {} fail: {}",
                    file_id_str, err))?;
                let local_path_str = String::from_str(subcommand.value_of("local_path").unwrap()).unwrap();
                let local_path = PathBuf::from_str(&local_path_str.as_str()).unwrap();
                let timeout = u32::from_str(subcommand.value_of("timeout").unwrap()).unwrap();

                Ok(Self::GetFile(DebugCommandGetFile {
                    remotes, 
                    timeout, 
                    file_id, 
                    local_path
                }))
            },
            "put_chunk" => {
                let subcommand = params.subcommand_matches("put_chunk").unwrap();
                let local_path_str = String::from_str(subcommand.value_of("local_path").unwrap()).unwrap();
                let local_path = PathBuf::from_str(&local_path_str.as_str()).unwrap();

                Ok(Self::PutChunk(DebugCommandPutChunk {
                    local_path
                }))
            },
            "put_file" => {
                let subcommand = params.subcommand_matches("put_file").unwrap();
                let local_path_str = String::from_str(subcommand.value_of("local_path").unwrap()).unwrap();
                let local_path = PathBuf::from_str(&local_path_str.as_str()).unwrap();

                Ok(Self::PutFile(DebugCommandPutFile {
                    local_path,
                }))
            },
            "sn_conn_status" => {
                let subcommand = params.subcommand_matches("sn_conn_status").unwrap();
                let timeout = u64::from_str(subcommand.value_of("timeout").unwrap()).unwrap();
                Ok(Self::SnConnStatus(DebugCommandSnConnStatus {
                    timeout_sec: timeout,
                }))
            },
            "bench_datagram" => {
                let subcommand = params.subcommand_matches("bench_datagram").unwrap();
                let remote = remote_device(stack, subcommand.value_of("remote").unwrap()).await
                    .map_err(|err| format!("load remote desc {} failed for {}\r\n", subcommand.value_of("remote").unwrap(), err))?;
                let plaintext = u32::from_str(subcommand.value_of("plaintext").unwrap()).unwrap();
                let timeout = u64::from_str(subcommand.value_of("timeout").unwrap()).unwrap();

                Ok(Self::BenchDatagram(DebugCommandBenchDatagram {
                    remote,
                    timeout: Duration::from_secs(timeout),
                    plaintext: plaintext != 0
                }))
            }
            _ => {
                Err(format!("invalid subcommand {}\r\n", subcommand))
            }
        }
    } 
}

pub struct DebugCommandTest {}

pub struct DebugCommandBenchDatagram {
    pub remote: Device, 
    pub timeout: Duration,
    pub plaintext: bool,
}

pub struct DebugCommandPing {
    pub remote: Device, 
    pub count: u32, 
    pub timeout: Duration
}

#[derive(Clone, Debug)]
pub struct DebugCommandNc {
    pub remote: Device, 
    pub port: u16, 
    pub timeout: Duration,
    pub bench: u32,
    pub task_num: u32,
}

pub struct DebugCommandGetChunk {
    pub remotes: Vec<DeviceDesc>, 
    pub timeout: u32,
    pub chunk_id: ChunkId,
    pub local_path: PathBuf,
}

pub struct DebugCommandGetFile {
    pub remotes: Vec<DeviceId>, 
    pub timeout: u32,
    pub file_id: File,
    pub local_path: PathBuf,
}

pub struct DebugCommandPutChunk {
    pub local_path: PathBuf,
}

pub struct DebugCommandPutFile {
    pub local_path: PathBuf,
}

pub struct DebugCommandSnConnStatus {
    pub timeout_sec: u64,
}

async fn remote_device(
    stack: &Stack, 
    str: &str) -> BuckyResult<Device> {
    if let Ok(device_id) = DeviceId::from_str(str) {
        if let Some(device) = stack.device_cache().get(&device_id).await {
            Ok(device)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "not found"))
        }
    } else {
        let path = Path::new(str);
        if !path.exists() {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "not found"))
        } else {
            let mut buf = vec![];
            let (device, _) = Device::decode_from_file(&path, &mut buf)?;
            let device_id = device.desc().device_id();
            if stack.device_cache().get(&device_id).await.is_none() {
                stack.device_cache().add(&device_id, &device);
            }
            Ok(device)
        }
    }
} 













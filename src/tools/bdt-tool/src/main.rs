use std::{
    io::{Read}, 
    path::Path, 
    str::FromStr, 
    time::Duration, 
    net::Shutdown,
};
use clap::{
    App, 
    SubCommand, 
    Arg
};
use async_std::{
    future,
};

use cyfs_base::*;
use cyfs_bdt::*;
use log::*;

mod sn_bench;
use crate::sn_bench::*;

fn load_dev_by_path(path: &str) -> Option<Device> {
    let desc_path = Path::new(path);
    if desc_path.exists() {
        let mut file = std::fs::File::open(desc_path).unwrap();
        let mut buf = Vec::<u8>::new();
        let _ = file.read_to_end(&mut buf).unwrap();
        let (device, _) = Device::raw_decode(buf.as_slice()).unwrap();
    
        Some(device)
    } else {
        None
    }
}

fn load_dev_vec(path: &str) -> Option<Vec<Device>> {
    let mut dev_vec = Vec::new();
    match load_dev_by_path(path) {
        Some(dev) => {
            dev_vec.push(dev);

            Some(dev_vec)
        },
        _ => None
    }
}

fn load_sn(sns: Vec<&str>) -> Option<Vec<Device>> {
    let mut dev_vec = Vec::new();

    if sns.len() == 0 {
        match load_dev_by_path("sn-miner.desc") {
            Some(dev) => {
                dev_vec.push(dev);
                Some(dev_vec)
            },
            _ => None
        }
    } else {
        for sn in sns {
            let dev = load_dev_by_path(sn).unwrap();
            dev_vec.push(dev);
        }

        Some(dev_vec)
    }
}

fn loger_init(log_level: &str, name: &str) {
    if log_level != "none" {
        cyfs_debug::CyfsLoggerBuilder::new_app(name)
            .level(log_level)
            .console(log_level)
            .build()
            .unwrap()
            .start();

        cyfs_debug::PanicBuilder::new(name, name)
        .exit_on_panic(true)
        .build()
        .start();
    }
}

pub fn command_line() -> clap::App<'static, 'static> {
    App::new("bdt-tool")
        .about("bdt tool")
        .arg(Arg::with_name("ep").long("ep").multiple(true).takes_value(true).help("local endpoint"))
        .arg(Arg::with_name("udp_sn_only").long("udp_sn_only").takes_value(false).default_value("0").help("udp sn only"))
        .arg(Arg::with_name("log_level").long("log_level").default_value("none").help("log level: none/info/debug/warn/error"))
        .arg(Arg::with_name("device_cache").long("device_cache").default_value("").help("device cache"))
        .arg(Arg::with_name("sn").long("sn").multiple(true).default_value("").help("sn desc file"))
        .arg(Arg::with_name("cmd").long("cmd").takes_value(false).help("sn desc file"))
        .subcommand(SubCommand::with_name("ping")
            .arg(Arg::with_name("remote").required(true))
            .arg(Arg::with_name("count").required(true))
            .arg(Arg::with_name("timeout").required(true))
        )
        .subcommand(SubCommand::with_name("nc")
            .arg(Arg::with_name("remote").required(true))
            .arg(Arg::with_name("port").required(true))
        )
        .subcommand(SubCommand::with_name("sn_bench_ping")
            .arg(Arg::with_name("remote").required(true))
            .arg(Arg::with_name("port").required(true))
        )
        .subcommand(SubCommand::with_name("sn_bench_call")
            .arg(Arg::with_name("remote").required(true))
            .arg(Arg::with_name("port").required(true))
        )
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
            } else {
            }
            Ok(device)
        }
    }
}

#[async_std::main]
async fn main() {
    //
    let cmd_line = std::env::args().collect::<Vec<String>>().join(" ");
    let matches = command_line().get_matches();

    let log_level = matches.value_of("log_level").unwrap();
    let udp_sn_only = u16::from_str(matches.value_of("udp_sn_only").unwrap()).unwrap();

    let cmd_params = command_line().get_matches_from_safe(cmd_line.split(" "))
        .map_err(|err| err.message).unwrap();
    let subcommand = cmd_params.subcommand_name().ok_or_else(|| "no subcommand\r\n".to_string()).unwrap();

    let mut endpoints = vec![];
    for ep in matches.values_of("ep").unwrap() {
        if let Ok(ep) = Endpoint::from_str(ep) {
            endpoints.push(ep);
        } else {
            println!("invalid endpoint {}", ep);
            return;
        }
    }

    let mut sns = vec![];
    for sn in matches.values_of("sn").unwrap() {
        if sn.len() != 0 {
            sns.push(sn);
        }
    }
    let sns = load_sn(sns);

    match subcommand {
        "sn_bench_ping" => {
            let subcommand = cmd_params.subcommand_matches("sn_bench_ping").unwrap();
            let device_load = subcommand.value_of("load").unwrap_or("");
            let device_num = u64::from_str(subcommand.value_of("device").unwrap_or("1000")).unwrap();
            let interval_ms = u64::from_str(subcommand.value_of("interval").unwrap_or("1000")).unwrap();
            let timeout_sec = u64::from_str(subcommand.value_of("timeout").unwrap_or("3")).unwrap();
            let bench_time = u64::from_str(subcommand.value_of("time").unwrap_or("60")).unwrap();

            let ping_exception = SnBenchPingException::default();
            let result = sn_bench_ping(
                device_num, device_load, 
                sns, endpoints, bench_time,
                interval_ms, 
                timeout_sec,
                ping_exception).await.unwrap();

            result.show();

            return;
        },
        "sn_bench_call" => {
            let subcommand = cmd_params.subcommand_matches("sn_bench_call").unwrap();
            let device_load = subcommand.value_of("load").unwrap_or("");
            let device_num = u64::from_str(subcommand.value_of("device").unwrap_or("1000")).unwrap();
            let interval_ms = u64::from_str(subcommand.value_of("interval").unwrap_or("1000")).unwrap();
            let timeout_sec = u64::from_str(subcommand.value_of("timeout").unwrap_or("3")).unwrap();
            let bench_time = u64::from_str(subcommand.value_of("time").unwrap_or("60")).unwrap();

            let ping_exception = SnBenchPingException::default();
            let result = sn_bench_call(
                device_num, device_load, 
                sns, endpoints, bench_time,
                interval_ms, 
                timeout_sec,
                ping_exception).await.unwrap();

            result.show();

            return;
        },
        _ => {}
    }

    //load device
    let desc_path = Path::new("deamon.desc");
    let sec_path = Path::new("deamon.sec");
    if !sec_path.exists() {
        println!("deamon.desc not exists, generate new one");

        let (device, private_key) = create_device(sns.clone(), endpoints.clone());

        if let Err(err) = device.encode_to_file(&desc_path, false) {
            println!("generate deamon.desc failed for {}",  err);
            return;
        } 
        if let Err(err) = private_key.encode_to_file(&sec_path, false) {
            println!("generate deamon.sec failed for {}",  err);
            return;
        }
    }
    if !desc_path.exists() {
        println!("{:?} not exists", desc_path);
        return;
    }
    let device = {
        let mut buf = vec![];
        Device::decode_from_file(&desc_path, &mut buf).map(|(d, _)| d)
    }; 
    if let Err(err) = device {
        println!("load {:?} failed for {}", desc_path, err);
        return;
    } 
    let mut device = device.unwrap();
    info!("device={:?}", device);

    let private_key = {
        let mut buf = vec![];
        PrivateKey::decode_from_file(&sec_path, &mut buf).map(|(k, _)| k)
    }; 

    if let Err(err) = private_key {
        println!("load {:?} failed for {}", sec_path, err);
        return;
    } 
    let private_key = private_key.unwrap();

    let deamon_id = device.desc().device_id();
    let deamon_name = format!("bdt-tool-{}", deamon_id);
    loger_init(log_level, deamon_name.as_str());

    let device_endpoints = device.mut_connect_info().mut_endpoints();
    device_endpoints.clear();
    for ep in endpoints {
        device_endpoints.push(ep);
    }

    //init stack
    let mut params = StackOpenParams::new(deamon_name.as_str());
    let sns2 = sns.clone();
    params.known_sn = sns;
    if udp_sn_only != 0 {
        params.config.interface.udp.sn_only = true;
    } else {
        params.config.interface.udp.sn_only = false;
    }

    let stack = Stack::open(
        device, 
        private_key, 
        params).await;
        
    if let Err(err) = stack {
        println!("open stack failed for {}", err);
        return ;
    }

    let stack = stack.unwrap();

    if sns2.is_some() {
        stack.reset_sn_list(sns2.unwrap());
    }

    match future::timeout(
        Duration::from_secs(5),
        stack.sn_client().ping().wait_online(),
    ).await {
        Ok(res) => {
            match res {
                Ok(res) => {
                    match res {
                        SnStatus::Online => {
                        },
                        _ => {
                            println!("sn offline!");
                        }
                    }
                },
                Err(e) => {
                    println!("connect sn err={}", e);
                }
            }
        },
        Err(e) => {
            println!("wait_online err={}", e);
        }
    }

    if let Some(device_cache) = matches.value_of("device_cache") {
        if device_cache.len() > 0 {
            let dev = load_dev_by_path(device_cache).unwrap();
            let device_id = dev.desc().device_id();
            stack.device_cache().add(&device_id, &dev);
        }
    }

    //
    match subcommand {
        "ping" => {
            let subcommand = cmd_params.subcommand_matches("ping").unwrap();
            let remote = remote_device(&stack, subcommand.value_of("remote").unwrap()).await
                .map_err(|err| format!("load remote desc {} failed for {}\r\n", subcommand.value_of("remote").unwrap(), err)).unwrap();
            let count = u32::from_str(subcommand.value_of("count").unwrap()).unwrap();
            let timeout = u64::from_str(subcommand.value_of("timeout").unwrap()).unwrap();

            let pinger = cyfs_bdt::debug::Pinger::open(stack.clone().to_weak()).unwrap();
            for _ in 0..count {
                match pinger.ping(remote.clone(), Duration::from_secs(timeout), "debug".as_ref()).await {
                    Ok(rtt) => {
                        match rtt {
                            Some(rtt) => {
                                println!("ping success, rtt is {:.2} ms", rtt as f64 / 1000.0);
                            },
                            None => {
                                println!("connected, but ping's seq mismatch");
                            }
                        }
                    },
                    Err(e) => {
                        println!("ping err={}", e);
                    }
                }
            }

        },
        "nc" => {
            let subcommand = cmd_params.subcommand_matches("nc").unwrap();
            let remote = remote_device(&stack, subcommand.value_of("remote").unwrap()).await
                .map_err(|err| format!("load remote desc {} failed for {}\r\n", subcommand.value_of("remote").unwrap(), err)).unwrap();
            let port = u16::from_str(subcommand.value_of("port").unwrap()).unwrap();
            let question = b"question?";
            match stack.stream_manager().connect(
                port,
                question.to_vec(), 
                BuildTunnelParams {
                    remote_const: remote.desc().clone(), 
                    remote_sn: None, 
                    remote_desc: Some(remote.clone())
            }).await {
                Ok(conn) => {
                    println!("connect vport={} success!", port);
                    let _ = conn.shutdown(Shutdown::Both);
                },
                Err(err) => {
                    println!("connect vport={} fail, err={}", port, err);
                }
            }
        },
        _ => {
            println!("unspport cmd {}", subcommand);
        }
    }
}

use std::{io::Read, path::Path, str::FromStr, net::Shutdown, time::Duration};
use async_std::{
    task,
    stream::StreamExt,
    io::{prelude::{ReadExt}, WriteExt},
    future,
};
use clap::{App, Arg};
use cyfs_base::*;
use cyfs_bdt::*;
use log::*;

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

fn load_sn() -> Option<Vec<Device>> {
    load_dev_vec("sn-miner.desc")
}

#[async_std::main]
async fn main() {
    let matches = App::new("bdt-debuger-deamon").version("1.0.0").about("deamon of bdt stack for debuger")
                        // .arg(Arg::with_name("desc").help("desc file of local device"))
                        // .arg(Arg::with_name("secret").help("private key file of local device"))
                        // .arg(Arg::with_name("sn").help("sn desc file to ping"))
                        .arg(Arg::with_name("listen").long("listen").takes_value(true).help("listen stream on vport"))
                        .arg(Arg::with_name("local").long("local").short("l").takes_value(true).default_value("127.0.0.1").help("debug command local ip to listen"))
                        .arg(Arg::with_name("port").long("port").short("p").takes_value(true).default_value("12345").help("debug command port to listen"))
                        .arg(Arg::with_name("ep").long("ep").multiple(true).takes_value(true).help("local endpoint"))
                        .arg(Arg::with_name("sn_conn_timeout").takes_value(true).default_value("0").help("sn connect timeout"))
                        .arg(Arg::with_name("active_pn").long("active_pn").takes_value(true).default_value("").help("active pn"))
                        .arg(Arg::with_name("passive_pn").long("passive_pn").takes_value(true).default_value("").help("passive pn"))
                        .arg(Arg::with_name("device_cache").long("device_cache").takes_value(true).default_value("").help("device cache"))
                        .arg(Arg::with_name("quiet").long("quiet").takes_value(false).default_value("0").help("quiet mode"))
                        .get_matches();

    let mut endpoints = vec![];
    for ep in matches.values_of("ep").unwrap() {
        if let Ok(ep) = Endpoint::from_str(ep) {
            endpoints.push(ep);
        } else {
            println!("invalid endpoint {}", ep);
            return;
        }
    }

    let quiet = u16::from_str(matches.value_of("quiet").unwrap()).unwrap();

    let sns = load_sn();

    let default_desc_path = Path::new("deamon.desc");
    let default_sec_path = Path::new("deamon.sec");
    if !default_desc_path.exists() || !default_sec_path.exists() {
        println!("deamon.desc not exists, generate new one");
        let private_key = PrivateKey::generate_rsa(1024).unwrap();
        let public_key = private_key.public();

        let sn_list = match sns.as_ref() {
            Some(sns) => {
                let mut sn_list = Vec::new();
                for sn in sns.iter() {
                    println!("sn_list push={}", sn.desc().device_id());
                    sn_list.push(sn.desc().device_id());
                }
                sn_list
            },
            None => vec![],
        };

        let device = Device::new(
            None,
            UniqueId::default(),
            endpoints.clone(),
            sn_list,
            vec![],
            public_key,
            Area::default(), 
            DeviceCategory::PC
        ).build();

        if let Err(err) = device.encode_to_file(&default_desc_path, false) {
            println!("generate deamon.desc failed for {}",  err);
            return;
        } 
        if let Err(err) = private_key.encode_to_file(&default_sec_path, false) {
            println!("generate deamon.sec failed for {}",  err);
            return;
        }
    }
    let desc_path = matches.value_of("desc").map(|s| Path::new(s)).unwrap_or(default_desc_path);
    let sec_path = matches.value_of("secret").map(|s| Path::new(s)).unwrap_or(default_sec_path);
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
    let deamon_name = format!("bdt-debuger-deamon-{}", deamon_id);
    if quiet == 0 {
        cyfs_debug::CyfsLoggerBuilder::new_app(deamon_name.as_str())
            .level("info")
            .console("info")
            .build()
            .unwrap()
            .start();
    }
    cyfs_debug::PanicBuilder::new(deamon_name.as_str(), deamon_name.as_str())
        .exit_on_panic(true)
        .build()
        .start();

    let device_endpoints = device.mut_connect_info().mut_endpoints();
    device_endpoints.clear();
    for ep in endpoints {
        device_endpoints.push(ep);
    }

    let local = matches.value_of("local").unwrap();
    let port = u16::from_str(matches.value_of("port").unwrap());
    if port.is_err() {
        println!("invalid arg port");
        return;
    }
    let port = port.unwrap();

    let chunk_store = MemChunkStore::new();
    let mut params = StackOpenParams::new(deamon_name.as_str());
    params.config.debug = Some(cyfs_bdt::debug::Config {
        local: local.to_string(),
        port,
        chunk_store: chunk_store.clone(),
    });
    let sns2 = sns.clone();
    params.known_sn = sns;
    params.config.interface.udp.sn_only = false;

    if let Some(active_pn) = matches.value_of("active_pn") {
        params.active_pn = load_dev_vec(active_pn);
    }
    if let Some(passive_pn) = matches.value_of("passive_pn") {
        params.passive_pn = load_dev_vec(passive_pn);
    }
    params.chunk_store = Some(Box::new(chunk_store.clone()));

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
                        SnStatus::Online => {},
                        _ => {
                            println!("sn offline");
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

    if let Some(vport) = matches.value_of("listen") {
        let vport = u16::from_str(vport);
        if vport.is_err() {
            println!("invalid arg listen");
            return;
        }
        let vport = vport.unwrap();
        let listener = stack.stream_manager().listen(vport);        
        if let Err(err) = listener {
            println!("listen stream on port {} failed for {}", vport, err);
            return;
        }
        println!("listen stream on port {}", vport);
        let listener = listener.unwrap();
        task::spawn(async move {
            let mut incoming = listener.incoming();
            loop {
                if let Some(stream) = incoming.next().await {
                    if let Ok(mut stream) = stream {
                        task::spawn(async move {
                            println!("question len={} content={:?}", 
                                stream.question.len(), String::from_utf8(stream.question).expect(""));

                            let answer = b"answer!";
                            let _ = stream.stream.confirm(&answer.to_vec()).await;

                            let mut read_buf = [0; 128];
                            if let Ok(n) = stream.stream.read(&mut read_buf).await {
                                println!("read len={} data={}", n, String::from_utf8(read_buf[..n].to_vec()).expect(""));
                            }

                            let write_buf = b"abcdefg";
                            if let Ok(n) = stream.stream.write(write_buf).await {
                                println!("write len={}", n);
                            }

                            task::spawn(async move {
                                let mut buf = vec![0u8; 2048];
                                let mut total = 0;
                                loop {
                                    match stream.stream.read(&mut buf).await {
                                        Ok(n) => {
                                            total += n;
                                            if n == 0 {
                                                break;
                                            }
                                        },
                                        Err(e) => {
                                            println!("read err={}", e);
                                            break;
                                        }
                                    }
                                }
                                task::sleep(std::time::Duration::from_millis(200)).await;
                                match stream.stream.shutdown(Shutdown::Both) {
                                    Ok(_) => println!("shutdown ok, total={}", total),
                                    Err(e) => println!("shutdown err: {:?}", e),
                                }
                            });
                        });
                    }
                }
            }
        });
    }

    println!("stack debug deamon running...");

    async_std::future::pending::<()>().await;
}

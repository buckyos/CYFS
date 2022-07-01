use std::{io::Read, path::Path, str::FromStr, net::Shutdown};
use async_std::{
    task,
    stream::StreamExt,
    io::prelude::{ReadExt},
};
use clap::{App, Arg};
use cyfs_base::*;
use cyfs_bdt::{self, Stack, StackOpenParams};

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
                        .arg(Arg::with_name("active_pn").takes_value(true).default_value("").help("active pn"))
                        .arg(Arg::with_name("passive_pn").takes_value(true).default_value("").help("passive pn"))
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

    let sns = load_sn();

    let default_desc_path = Path::new("deamon.desc");
    let default_sec_path = Path::new("deamon.sec");
    if !default_desc_path.exists() || !default_sec_path.exists() {
        println!("deamon.desc not exists, generate new one");
        let private_key = PrivateKey::generate_rsa(1024).unwrap();
        let public_key = private_key.public();
    
        let device = Device::new(
            None,
            UniqueId::default(),
            endpoints.clone(),
            vec![],
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
    cyfs_debug::CyfsLoggerBuilder::new_app(deamon_name.as_str())
        .level("info")
        .console("info")
        .build()
        .unwrap()
        .start();

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

    let mut params = StackOpenParams::new(deamon_name.as_str());
    params.config.debug = Some(cyfs_bdt::debug::Config {
        local: local.to_string(),
        port
    });
    params.known_sn = sns;

    if let Some(active_pn) = matches.value_of("active_pn") {
        params.active_pn = load_dev_vec(active_pn);
    }
    if let Some(passive_pn) = matches.value_of("passive_pn") {
        params.passive_pn = load_dev_vec(passive_pn);
    }

    let stack = Stack::open(
        device, 
        private_key, 
        params).await;
        
    if let Err(err) = stack {
        println!("open stack failed for {}", err);
        return;
    }

    let stack = stack.unwrap();

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
                        println!("question len={} content={:?}", 
                            stream.question.len(), String::from_utf8(stream.question).expect(""));

                        let _ = stream.stream.confirm(b"accepted!".as_ref()).await;

                        task::spawn(async move {
                            let mut buf = vec![];
                            match stream.stream.read_to_end(&mut buf).await {
                                Ok(len) => {
                                    println!("read data success. len={} data={}", 
                                        len, String::from_utf8(buf[..len].to_vec()).expect(""));
                                },
                                Err(e) => {
                                    println!("read data err: {}", e);
                                }
                            }

                            let _ = stream.stream.shutdown(Shutdown::Both);
                        });
                    }
                }
            }
        });
    }

    println!("stack debug deamon running...");

    async_std::future::pending::<()>().await;
}
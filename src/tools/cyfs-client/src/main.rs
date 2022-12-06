use std::env::args;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;

mod named_data_client;
mod ffs_client_util;
mod meta_helper;
mod actions;

use clap::{App, SubCommand, Arg, ArgMatches};

use crate::actions::{put, get, get_by_id, create, upload};
use crate::named_data_client::{NamedCacheClient, NamedCacheClientConfig};

use log::*;
use cyfs_base::{PrivateKey, Device, File, FileDecoder, StandardObject, RawConvertTo, BuckyResult, RawFrom, Area};
use cyfs_lib::{NONGetObjectOutputRequest, SharedCyfsStack, UtilGetDeviceStaticInfoOutputRequest};
use cyfs_meta_lib::MetaMinerTarget;

extern crate log;

fn get_device_desc(matches: &ArgMatches, name: &str) -> Option<(Device, PrivateKey)> {
    if let Some(path) = matches.value_of(name) {
        match cyfs_util::get_device_from_file(&Path::new(path).with_extension("desc"), &Path::new(path).with_extension("sec")) {
            Ok(ret) => {
                debug!("sec: {}", ret.1.to_hex().unwrap());
                Some(ret)
            },
            Err(e) => {
                error!("read desc from {} fail, err {}", path, e);
                None
            }
        }
    } else {
        None
    }
}

fn get_desc(matches: &ArgMatches, name: &str) -> Option<(StandardObject, PrivateKey)> {
    if let Some(path) = matches.value_of(name) {
        match cyfs_util::get_desc_from_file(&Path::new(path).with_extension("desc"), &Path::new(path).with_extension("sec")) {
            Ok(ret) => Some(ret),
            Err(e) => {
                error!("read desc from {} fail, err {}", path, e);
                None
            }
        }
    } else {
        None
    }
}

async fn get_sn_list(stack: &SharedCyfsStack) -> BuckyResult<Vec<Device>> {
    stack.wait_online(Some(Duration::from_secs(5))).await?;
    let info = stack.util().get_device_static_info(UtilGetDeviceStaticInfoOutputRequest::new()).await?;
    info!("get sn list from runtime: {:?}", info.info.known_sn_list);
    let mut devices = vec![];
    for sn_id in &info.info.known_sn_list {
        let resp = stack.non_service().get_object(NONGetObjectOutputRequest::new_noc(sn_id.object_id().clone(), None)).await?;
        devices.push(Device::clone_from_slice(&resp.object.object_raw)?);
    }

    Ok(devices)
}

async fn sn_list(matches: &ArgMatches<'_>) -> (Vec<Device>, Option<Area>) {
    if matches.is_present("stack_sn") {
        let stack = SharedCyfsStack::open_runtime(None).await.unwrap();
        let area = stack.local_device_id().object_id().info().into_area();
        info!("get area from runtime: {:?}", area);
        (get_sn_list(&stack).await.unwrap_or_else(|e| {
            error!("get sn list from runtime err {}, use built-in sn list", e);
            cyfs_util::get_builtin_sn_desc().as_slice().iter().map(|(_, device)| device.clone()).collect()
        }), area)
    } else {
        (cyfs_util::get_builtin_sn_desc().as_slice().iter().map(|(_, device)| device.clone()).collect(), None)
    }
}

async fn main_run() {
    cyfs_debug::CyfsLoggerBuilder::new_service("cyfs-client")
        .level("info")
        .console("info")
        .enable_bdt(Some("warn"), Some("warn"))
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("cyfs-tools", "cyfs-client").build().start();
    let default_target = MetaMinerTarget::default().to_string();
    let meta_arg = Arg::with_name("meta_target").short("m").long("meta_target").takes_value(true).default_value(&default_target).help("meta target");

    let put_command = SubCommand::with_name("put")
        .about("put file to owner`s OOD")
        .arg(Arg::with_name("file").required(true).help("set the file path to put").index(1))
        .arg(Arg::with_name("owner").short("o").long("owner").takes_value(true).help("owner desc and sec file path, exclude extension"))
        .arg(Arg::with_name("chunk_size").short("c").long("chunk_size").default_value("8192").help("file chunk size"))
        .arg(Arg::with_name("file_id").short("f").long("file_id").takes_value(true).help("save file id to path(optional)"))
        .arg(Arg::with_name("url_file").long("url_file").takes_value(true).help("save ffs url to path(optional)"))
        .arg(Arg::with_name("desc").short("d").long("desc").takes_value(true).help("bdt init desc, use on own risk"))
        .arg(Arg::with_name("output").long("output").takes_value(true).help("chunk save path"))
        .arg(Arg::with_name("stack_sn").long("use-stack-sn").hidden(true).help("use local runtime`s newest sn list"))
        .arg(meta_arg.clone());

    let matches = App::new("cyfs-client").version(cyfs_base::get_version())
        .subcommand(put_command.clone())
        .subcommand(SubCommand::with_name("get")
            .about("get file from OOD")
            .arg(Arg::with_name("url").required(true).help("file url to get").index(1))
            .arg(Arg::with_name("dest").help("dest file path").index(2))
            .arg(Arg::with_name("desc").short("d").long("desc").takes_value(true).help("bdt init desc, use on own risk"))
            .arg(Arg::with_name("stack_sn").long("use-stack-sn").hidden(true).help("use local runtime`s newest sn list"))
            .arg(meta_arg.clone())
        )
        .subcommand(put_command.clone().name("create").about("create filedesc, only for test"))
        .subcommand(SubCommand::with_name("getbyid")
            .about("get file by fileid")
            .arg(Arg::with_name("fileid").required(true).help("file id to get").index(1))
            .arg(Arg::with_name("dest").help("dest file path").index(2))
            .arg(Arg::with_name("desc").short("d").long("desc").takes_value(true).help("bdt init desc, use on own risk"))
            .arg(Arg::with_name("stack_sn").long("use-stack-sn").hidden(true).help("use local runtime`s newest sn list"))
            .arg(meta_arg.clone())
        )
        .subcommand(SubCommand::with_name("upload")
            .about("upload desc to meta")
            .arg(Arg::with_name("desc_file").required(true).help("desc file upload to meta"))
            .arg(Arg::with_name("owner").required(true).help("owner desc and sec file path, exclude extension").index(2))
            .arg(meta_arg.clone())
        )
        .subcommand(SubCommand::with_name("extract")
            .about("extract cyfs url to owner, id, inner_path")
            .arg(Arg::with_name("url").index(1).required(true).takes_value(true).help("cyfs url"))
            .arg(meta_arg.clone())
        )
        .get_matches();


    match matches.subcommand() {
        ("put", Some(matches)) => {
            //let matches = matches.clone();
            let file = PathBuf::from(matches.value_of("file").unwrap());
            let chunk_size = matches.value_of("chunk_size").unwrap().parse::<u32>().unwrap() * 1024;
            let file_id = matches.value_of("file_id").map(PathBuf::from);
            let url_file = matches.value_of("url_file").map(PathBuf::from);

            let mut client = NamedCacheClient::new();
            let desc = get_device_desc(&matches, "desc");
            let meta_target = matches.value_of("meta_target").map(|s|
                MetaMinerTarget::from_str(&s).unwrap_or(MetaMinerTarget::default()));
            let (sn_list, area) = sn_list(matches).await;
            let mut config = NamedCacheClientConfig::default();
            config.desc = desc;
            config.meta_target = meta_target;
            config.sn_list = Some(sn_list);
            config.area = area;
            client.init(config).await.unwrap();

            if let Some((owner_desc, secret)) = get_desc(&matches, "owner") {
                info!("@put...");
                async_std::task::spawn(async move {
                    match put(&mut client, &file, &owner_desc, &secret, chunk_size, url_file, file_id, true).await {
                        Ok((url, time)) => {
                            info!("put success, ffs url: {}", url);
                            info!("put total use {} secs", time.as_secs());
                        }
                        _ => {
                            std::process::exit(1);
                        }
                    };
                }).await;
            } else {
                std::process::exit(1);
            }
        },
        ("get", Some(matches)) => {
            let url = matches.value_of("url").unwrap().to_owned();
            let dest_path = PathBuf::from(matches.value_of("dest").unwrap());
            let mut client = NamedCacheClient::new();
            let desc = get_device_desc(matches, "desc");
            let meta_target = matches.value_of("meta_target").map(|s|
                MetaMinerTarget::from_str(&s).unwrap_or(MetaMinerTarget::default()));
            let (sn_list, area) = sn_list(matches).await;
            let mut config = NamedCacheClientConfig::default();
            config.desc = desc;
            config.meta_target = meta_target;
            config.sn_list = Some(sn_list);
            config.area = area;
            client.init(config).await.unwrap();
            async_std::task::spawn(async move {
                if get(&client, &url, &dest_path).await.is_err() {
                    std::process::exit(1);
                };
            }).await;

        },

        ("getbyid", Some(matches)) => {
            let fileid = matches.value_of("fileid").unwrap().to_owned();
            let dest_path = PathBuf::from(matches.value_of("dest").unwrap_or(&fileid));

            let mut client = NamedCacheClient::new();
            let desc = get_device_desc(matches, "desc");
            let meta_target = matches.value_of("meta_target").map(|s|
                MetaMinerTarget::from_str(s).unwrap_or(MetaMinerTarget::default())
            );
            let (sn_list, area) = sn_list(matches).await;
            let mut config = NamedCacheClientConfig::default();
            config.desc = desc;
            config.meta_target = meta_target;
            config.sn_list = Some(sn_list);
            config.area = area;
            client.init(config).await.unwrap();
            async_std::task::spawn(async move {
                if get_by_id(&client, &fileid, &dest_path, None).await.is_err() {
                    std::process::exit(1);
                };
            }).await;

        },
        ("create", Some(matches)) => {
            let file = PathBuf::from(matches.value_of("file").unwrap());
            let chunk_size = matches.value_of("chunk_size").unwrap().parse::<u32>().unwrap() * 1024;
            let file_id = matches.value_of("file_id").map(PathBuf::from);
            let output = matches.value_of("output").map(PathBuf::from);

            match get_desc(matches, "owner") {
                Some((owner_desc, secret)) => {
                    async_std::task::spawn(async move {
                        if let Err(_) = create(&file, &owner_desc, &secret, chunk_size, file_id, output).await {
                            std::process::exit(1);
                        };
                    }).await;
                }
                None => {
                    std::process::exit(1);
                }
            }
        },

        ("upload", Some(matches)) => {
            let desc_file = Path::new(matches.value_of("desc_file").unwrap());
            let owner = Path::new(matches.value_of("owner").unwrap());
            let meta_target = matches.value_of("meta_target").map(str::to_string);
            match File::decode_from_file(desc_file, &mut vec![]) {
                Ok((desc, _)) => {
                    match cyfs_util::get_desc_from_file(&owner.with_extension("desc"), &owner.with_extension("sec")) {
                        Ok((owner_desc, secret)) => {
                            async_std::task::spawn(async move {
                                if upload(&owner_desc, &secret, &desc, meta_target).await.is_err() {
                                    std::process::exit(1);
                                }
                            }).await;

                        }
                        Err(e) => {
                            error!("get desc failed: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                Err(e) => {
                    error!("read desc from {} failed, err {}", desc_file.display(), e);
                    std::process::exit(1);
                }
            }
        },
        ("extract", Some(matches)) => {
            let mut client = NamedCacheClient::new();
            let meta_target = matches.value_of("meta_target").map(|s|
                MetaMinerTarget::from_str(s).unwrap_or(MetaMinerTarget::default()));
            let url = matches.value_of("url").unwrap().to_owned();
            let mut config = NamedCacheClientConfig::default();
            config.meta_target = meta_target;
            client.init(config).await.unwrap();
            async_std::task::spawn(async move {
                match client.extract_cyfs_url(&url).await {
                    Ok((owner, id, inner)) => {
                        println!("owner: {}\nid: {}\ninner: {}"
                                 , owner.map_or("None".to_owned(), |id|{id.to_string()})
                                 , &id
                                 , &inner)
                    }
                    Err(e) => {
                        error!("extract cyfs url err {}", e);
                    }
                }
            }).await;

        },
        v @ _ => {
            error!("unknown command: {}", v.0);
            std::process::exit(1);
        },
    };
}

fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run());
}
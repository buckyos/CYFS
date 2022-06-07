use clap::{App, SubCommand, Arg, ArgMatches};
use crate::common::{get_objid_from_str, get_caller_arg, get_desc_and_secret_from_matches, DEFAULT_DESC_PATH};
use log::*;
use std::path::Path;
use cyfs_meta_lib::{MetaClient};
use cyfs_base::*;
use cyfs_base_meta::*;
use std::convert::TryFrom;
use std::str::FromStr;

pub fn append_command<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
    app.subcommand(SubCommand::with_name("getdesc")
        .about("get desc from objectid or name")
        .arg(Arg::with_name("obj").index(1).takes_value(true).required(true)
            .help("objectid or name"))
        .arg(Arg::with_name("file").short("f").long("file").takes_value(true)
            .help("save desc to file"))
    )
    .subcommand(SubCommand::with_name("putdesc")
        .about("create or update desc on meta,exclude userdata")
        .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
        .arg(Arg::with_name("desc").long("desc").short("d").takes_value(true)
            .help("desc file send to meta, default caller`s desc"))
        .arg(Arg::with_name("value").short("v").long("value").default_value("0").takes_value(true).help("balance from caller to desc account when create"))
        .arg(Arg::with_name("price").index(1).takes_value(true).help("desc rent"))
        .arg(Arg::with_name("update").short("u").help("force update body time on put"))
        .arg(Arg::with_name("coin_id").index(2).takes_value(true).help("coin id"))
    )
    .subcommand(SubCommand::with_name("removedesc")
        .about("remove desc on meta")
        .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
        .arg(Arg::with_name("desc_id").long("desc_id").short("id").takes_value(true)
            .help("desc id will be removed, default caller`s id"))
    )
}

pub async fn match_command(matches: &ArgMatches<'_>, client: &MetaClient) -> BuckyResult<bool> {
    match matches.subcommand() {
        ("getdesc", Some(matches)) => {
            let objid = get_objid_from_str(matches.value_of("obj").unwrap(), &client).await?;
            let desc = client.get_desc(&objid).await?;
            match &desc {
                SavedMetaObject::Data(data) => {
                    let obj = AnyNamedObject::clone_from_slice(&data.data).unwrap();
                    info!("get desc {} : {}", &objid, obj.to_hex().unwrap());
                    if let Some(file) = matches.value_of("file") {
                        obj.encode_to_file(file.as_ref(), false).unwrap();
                    }
                },
                _ => {
                    if let Ok(standard_obj) = StandardObject::try_from(desc.clone()) {
                        info!("get desc {} : {}", &objid, standard_obj.to_hex().unwrap());
                        if let Some(file) = matches.value_of("file") {
                            standard_obj.encode_to_file(file.as_ref(), false).unwrap();
                        }
                    }
                }
            }
        }
        ("putdesc", Some(matches)) => {
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let mut send_desc;
            if let Some(desc) = matches.value_of("desc") {
                send_desc = AnyNamedObject::decode_from_file(desc.as_ref(), &mut Vec::new())?.0;
            } else {
                send_desc = AnyNamedObject::decode_from_file(&Path::new(matches.value_of("caller").unwrap()).with_extension("desc"), &mut Vec::new())?.0;
            }

            if matches.is_present("update") {
                send_desc.set_body_update_time(bucky_time_now());
            }

            let mut update = false;
            let send_id = send_desc.calculate_id();
            let send_meta_desc;
            match send_desc {
                AnyNamedObject::Standard(obj) => {
                    if let Ok(obj) = SavedMetaObject::try_from(obj.clone()) {
                        send_meta_desc = obj;
                    } else {
                        send_meta_desc = SavedMetaObject::Data(Data { id: obj.calculate_id(), data: obj.to_vec()? })
                    }
                }
                v @ _ => {
                    send_meta_desc = SavedMetaObject::Data(Data { id: v.calculate_id(), data: v.to_vec()? })
                }
            }

            // 先试着查询链上是否已有desc
            if let Ok(_) = client.get_desc(&send_id).await {
                info!("find id {} on meta, update", &send_id);
                update = true;

                // 如果是PeerDesc，那么把链上的userdata复制到send_desc上去
                //TODO
                // if let SavedMetaObject::Device(p) | SavedMetaObject::People(p) = desc {
                //     if let SavedMetaObject::Device(ref mut sp) | SavedMetaObject::People(ref mut sp) = send_desc {
                //         sp.user_data = p.user_data;
                //     }
                // }
            }

            // 更新或创建
            let hash;
            let price = matches.value_of("price").map(|p|p.parse::<u32>().unwrap());
            let coin_id = matches.value_of("coin_id").map(|p|p.parse::<u8>().unwrap());

            if update {
                hash = client.update_desc(&caller, &send_meta_desc, price, coin_id, &secret).await?;
            } else {
                let value = matches.value_of("value").map(|v|v.parse::<i64>().unwrap()).unwrap();
                hash = client.create_desc(&caller, &send_meta_desc, value, price.unwrap_or(0), coin_id.unwrap_or(0), &secret).await?;
            }

            info!("create/update desc {} success, txhash {}", &send_id, hash.to_string());
        }
        ("removedesc", Some(matches)) => {
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let desc_id;
            if let Some(desc) = matches.value_of("desc_id") {
                desc_id = ObjectId::from_str(desc)?;
            } else {
                desc_id = caller.calculate_id();
            }
            client.remove_desc(&caller, &desc_id, &secret).await?;
        }
        _ => {return Ok(true)}
    };
    Ok(false)
}

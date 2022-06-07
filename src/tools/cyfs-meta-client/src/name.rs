use clap::{App, SubCommand, Arg, ArgMatches};
use crate::common::{get_caller_arg, get_desc_and_secret_from_matches, get_objid_from_str, DEFAULT_DESC_PATH};
use std::net::IpAddr;
use std::str::FromStr;
use log::*;
use cyfs_base::{BuckyResult, NameLink, ObjectId};
use cyfs_meta_lib::{MetaClient};

pub fn append_command<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
    app.subcommand(SubCommand::with_name("bidname")
        .about("bid a name on meta chain")
        .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
        .arg(Arg::with_name("owner").short("o").long("owner").takes_value(true)
            .help("name owner, default = caller"))
        .arg(Arg::with_name("name").index(1).takes_value(true).required(true)
            .help("name want to bid"))
        .arg(Arg::with_name("bid_price").index(2).takes_value(true)
            .help("bid price"))
        .arg(Arg::with_name("rent").index(3).takes_value(true)
            .help("name rent price"))
    )
        .subcommand(SubCommand::with_name("getname")
            .about("get name info")
            .arg(Arg::with_name("name").index(1).takes_value(true).required(true)
                .help("name"))
        )
        .subcommand(SubCommand::with_name("namelink")
            .about("link name to an obj or name")
            .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
            .arg(Arg::with_name("name").index(1).takes_value(true).required(true)
                .help("name want to link"))
            .arg(Arg::with_name("type").short("t").long("type").takes_value(true).required(true)
                .possible_values(&["obj", "name", "ip"])
                .help("link type"))
            .arg(Arg::with_name("obj").index(2).takes_value(true).required(true)
                .help("obj to link"))
        )
        .subcommand(SubCommand::with_name("auctionname")
            .about("auction owned name")
            .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
            .arg(Arg::with_name("name").index(1).takes_value(true).required(true).help("name want to auction"))
            .arg(Arg::with_name("price").index(2).takes_value(true).required(true).help("starting price of auction"))
        )
        .subcommand(SubCommand::with_name("cancelauctionname")
            .about("cancel being auctioned name")
            .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
            .arg(Arg::with_name("name").index(1).takes_value(true).required(true).help("name is being auctioned"))
        )
        .subcommand(SubCommand::with_name("buybackname")
            .about("buy back being auctioned name because of rent arrears")
            .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
            .arg(Arg::with_name("name").index(1).takes_value(true).required(true).help("name want to buy back"))
        )
}
pub async fn match_command(matches: &ArgMatches<'_>, client: &MetaClient) -> BuckyResult<bool> {
    match matches.subcommand() {
        ("bidname", Some(matches)) => {
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let owner = match matches.value_of("owner") {
                None => None,
                Some(owner) => {
                    match get_objid_from_str(owner, &client).await {
                        Ok(id) => Some(id),
                        Err(_) => {
                            warn!("invalid owner, ignore");
                            None
                        },
                    }
                },
            };
            let name = matches.value_of("name").unwrap();
            let bid_price: u64 = matches.value_of("bid_price").unwrap().parse::<u64>().expect("need number");
            let rent: u32 = matches.value_of("rent").unwrap().parse::<u32>().expect("need number");
            let hash = client.bid_name(&caller, owner, matches.value_of("name").unwrap(), bid_price, rent, &secret).await?;
            info!("bid name {} success, txhash {}", name, hash.to_string());
        }
        ("getname", Some(matches)) => {
            let name = matches.value_of("name").unwrap();
            if let Some((info, state)) = client.get_name(name).await?{
                if info.owner.is_some() {
                    info!("find name {}, state {}, owner {}, {}", name, state as u8, info.owner.as_ref().unwrap(), info.record.link);
                } else {
                    info!("find name {}, state {}, owner none, {}", name, state as u8, info.record.link);
                }
            } else {
                info!("cannot find name {} on meta chain", name);
            }
        }
        ("namelink", Some(matches)) => {
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let name = matches.value_of("name").unwrap();
            if let Some((mut info, _)) = client.get_name(name).await?{
                let obj_str = matches.value_of("obj").unwrap();
                match matches.value_of("type").unwrap() {
                    "name" => {
                        info.record.link = NameLink::OtherNameLink(obj_str.to_owned())
                    }
                    "obj" => {
                        let obj = ObjectId::from_str(obj_str).map_err(|e| {
                            error!("invalid object id {}, err {}", obj_str, e);
                            e
                        })?;
                        info.record.link = NameLink::ObjectLink(obj)
                    }
                    "ip" => {
                        let ip = IpAddr::from_str(obj_str).map_err(|e| {
                            error!("{} is not an valid ipaddr, err {}", obj_str, e);
                            e
                        })?;
                        info.record.link = NameLink::IPLink(ip);
                    }
                    v@ _ => {
                        error!("link type {} invalid", v);
                        std::process::exit(1);
                    }
                }
                let txhash = client.update_name(&caller, name, info, 0, &secret).await?;
                info!("update name {} info, hash {}", name, txhash.to_string());
            } else {
                error!("cannot find name {} on meta chain", name);
            }
        }
        ("auctionname", Some(matches)) => {
            let name = matches.value_of("name").expect("must set name");
            let price = matches.value_of("price").expect("must set starting price").parse::<u64>().expect("need number");
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            match client.auction_name(&caller, name, price, &secret).await {
                Ok(hash) => {
                    info!("auction name:{}, hash {}", name, hash.to_string())
                },
                Err(e) => {
                    error!("auction name failed, err {}", e);
                }
            }
        }
        ("cancelauctionname", Some(matches)) => {
            let name = matches.value_of("name").expect("must set name");
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            match client.cancel_auction_name(&caller, name, &secret).await {
                Ok(hash) => {
                    info!("cancel auction name:{}, hash {}", name, hash.to_string())
                },
                Err(e) => {
                    error!("cancel auction name failed, err {}", e);
                }
            }
        }
        ("buybackname", Some(matches)) => {
            let name = matches.value_of("name").expect("must set name");
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            match client.buy_back_name(&caller, name, &secret).await {
                Ok(hash) => {
                    info!("buy back name:{}, hash {}", name, hash.to_string())
                },
                Err(e) => {
                    error!("buy back name failed, err {}", e);
                }
            }
        }
        _ => {return Ok(true)}
    };
    Ok(false)
}

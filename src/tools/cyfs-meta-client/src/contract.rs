use clap::{App, SubCommand, Arg, ArgMatches};
use crate::common::{get_caller_arg, get_desc_and_secret_from_matches, get_objid_from_str, DEFAULT_DESC_PATH};
use cyfs_base::{BuckyResult};
use cyfs_meta_lib::{MetaClient};
use log::*;

pub fn append_command<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
    app.subcommand(SubCommand::with_name("deploy")
        .about("deploy contract to meta chain, use eth create method")
        .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
        .arg(Arg::with_name("value").short("v").long("value").default_value("0")
            .help("balance from caller to contract account when deploy"))
        .arg(Arg::with_name("bin").short("b").long("bin").required(true).takes_value(true)
            .help("contract bin file path, generate from cyfs-solc or cyfs-solcjs"))
        .arg(Arg::with_name("abi").short("a").long("abi").takes_value(true)
            .help("contract abi file path, generate from cyfs-solc or cyfs-solcjs"))
        .arg(Arg::with_name("params").short("p").long("params").requires("abi").multiple(true).takes_value(true))
        .arg(Arg::with_name("maxfee").short("f").long("maxfee").default_value("0")
            .help("call max fee(gas limit)"))
    ).subcommand(SubCommand::with_name("call")
        .about("call contract function through transcation, like eth send")
        .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
        .arg(Arg::with_name("address").index(1).required(true).takes_value(true).help("contract address or cyfs name"))
        .arg(Arg::with_name("value").short("v").long("value").default_value("0").takes_value(true)
            .help("balance from caller to contract account when deploy"))
        .arg(Arg::with_name("abi").short("a").long("abi").required(true).takes_value(true)
            .help("contract abi file path, generate from cyfs-solc or cyfs-solcjs"))
        .arg(Arg::with_name("name").index(2).required(true).takes_value(true)
            .help("contract function name or signature"))
        .arg(Arg::with_name("params").short("p").long("params").multiple(true).takes_value(true))
        .arg(Arg::with_name("maxfee").short("f").long("maxfee").default_value("0")
            .help("call max fee(gas limit)"))
    ).subcommand(SubCommand::with_name("view")
        .about("call view contract function, like eth call")
        .arg(Arg::with_name("address").index(1).required(true).takes_value(true).help("contract address or cyfs name"))
        .arg(Arg::with_name("abi").short("a").long("abi").required(true).takes_value(true)
            .help("contract abi file path, generate from cyfs-solc or cyfs-solcjs"))
        .arg(Arg::with_name("name").index(2).required(true).takes_value(true)
            .help("contract function name or signature"))
        .arg(Arg::with_name("params").short("p").long("params").multiple(true).takes_value(true))
    ).subcommand(SubCommand::with_name("logs")
        .about("view contract logs, only support named event")
        .arg(Arg::with_name("address").index(1).required(true).takes_value(true).help("contract address or cyfs name"))
        .arg(Arg::with_name("name").index(2).required(true).takes_value(true).help("event name or sign"))
        .arg(Arg::with_name("abi").short("a").long("abi").required(true).takes_value(true)
            .help("contract abi file path, generate from cyfs-solc or cyfs-solcjs"))
        .arg(Arg::with_name("topic").short("t").long("topic").multiple(true).takes_value(true))
    )
}

pub async fn match_command(matches: &ArgMatches<'_>, client: &MetaClient) -> BuckyResult<bool> {
    match matches.subcommand() {
        ("deploy", Some(matches)) => {
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let init_code_str = std::fs::read_to_string(matches.value_of("bin").unwrap())?;
            let init_code = hex::decode(&init_code_str)?;

            let init_data = if let Some(abi) = matches.value_of("abi") {
                let abi = std::fs::read_to_string(abi)?;
                let params: Vec<String> = matches.values_of("params").map(|values|{
                    values.map(|v|v.to_owned()).collect()
                }).unwrap_or(vec![]);
                ethabi::encode_constructor(&abi, init_code, &params, true).unwrap()
            } else {
                init_code
            };
            let balance = matches.value_of("value").unwrap().parse::<i64>().map_err(|e|{
                error!("invalid balance amount, err {}", e);
                e
            })?;

            let max_fee = matches.value_of("maxfee").unwrap().parse::<u32>().map_err(|e|{
                error!("invalid maxfee amount, err {}", e);
                e
            })?;

            let hash = client.create_contract(&caller, &secret, balance as u64, init_data, 10, max_fee).await?;

            info!("deploy contract success, txhash {}", &hash);
        },
        ("call", Some(matches)) => {
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let address = get_objid_from_str(matches.value_of("address").unwrap(), &client).await?;
            let abi = std::fs::read_to_string(matches.value_of("abi").unwrap())?;
            let params: Vec<String> = matches.values_of("params").map(|values|{
                values.map(|v|v.to_owned()).collect()
            }).unwrap_or(vec![]);
            let init_data = ethabi::encode_input(&abi, matches.value_of("name").unwrap(), &params, true).unwrap();

            let balance = matches.value_of("value").unwrap().parse::<i64>().map_err(|e|{
                error!("invalid balance amount, err {}", e);
                e
            })?;

            let max_fee = matches.value_of("maxfee").unwrap().parse::<u32>().map_err(|e|{
                error!("invalid maxfee amount, err {}", e);
                e
            })?;

            let hash = client.call_contract(&caller, &secret, address, balance as u64, init_data, 10, max_fee).await?;

            info!("call contract success, txhash {}", &hash);
        },
        ("view", Some(matches)) => {
            let address = get_objid_from_str(matches.value_of("address").unwrap(), &client).await?;
            let abi = std::fs::read_to_string(matches.value_of("abi").unwrap())?;
            let params: Vec<String> = matches.values_of("params").map(|values|{
                values.map(|v|v.to_owned()).collect()
            }).unwrap_or(vec![]);
            let function = ethabi::load_function(&abi, matches.value_of("name").unwrap()).unwrap();
            let init_data = ethabi::encode_input_from_function(&function, &params, true).unwrap();

            let ret = client.view_contract(address, init_data).await?;
            let result_str = ethabi::decode_call_output_from_function(&function, ret.value).unwrap();
            info!("view ret {}, output {}", ret.ret, result_str);
        },
        ("logs", Some(matches)) => {
            let address = get_objid_from_str(matches.value_of("address").unwrap(), &client).await?;
            let abi = std::fs::read_to_string(matches.value_of("abi").unwrap())?;
            let params: Vec<Option<String>> = matches.values_of("params").map(|values|{
                values.map(|v|{
                    if v != "none" {
                        Some(v.to_owned())
                    } else {
                        None
                    }
                }).collect()
            }).unwrap_or(vec![]);

            let event = ethabi::load_event(&abi, matches.value_of("name").unwrap()).unwrap();
            let topics = ethabi::encode_topics_from_event(&event, params).unwrap();
            let from = matches.value_of("from").map(|s|s.parse().unwrap_or(0)).unwrap_or(0);
            let to = matches.value_of("to").map(|s|s.parse().unwrap_or(0)).unwrap_or(0);

            let rets = client.get_logs(address, topics, from, to).await?;

            for ret in rets {
                info!("get log:");
                let log = event.parse_log(ret.into()).unwrap();
                for param in log.params {
                    info!("\tname: {}, value: {}", param.name, param.value);
                }
            }
        }
        _ => {return Ok(true)}
    }

    Ok(false)
}

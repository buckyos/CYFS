use clap::{App, SubCommand, Arg, ArgMatches};
use crate::common::{get_caller_arg, get_desc_and_secret_from_matches, DEFAULT_DESC_PATH};
use log::*;
use cyfs_base::{BuckyResult, TxId};
use cyfs_meta_lib::{MetaClient};
use std::str::FromStr;

pub fn append_command<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
    app.subcommand(SubCommand::with_name("getreceipt")
        .about("get receipt for tx")
        .arg(Arg::with_name("tx").index(1).takes_value(true).required(true)
            .help("tx hash"))
        .arg(Arg::with_name("abi").short("a").long("abi").takes_value(true)
            .help("contract abi file path, generate from cyfs-solc or cyfs-solcjs"))
        .arg(Arg::with_name("name").short("n").long("name").takes_value(true).requires("abi")
            .help("contract function name or signature"))
    ).subcommand(SubCommand::with_name("setconfig")
        .about("set config to meta")
        .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
        .arg(Arg::with_name("key").index(1).takes_value(true).required(true).help("config key"))
        .arg(Arg::with_name("value").index(2).takes_value(true).required(true).help("config value"))
    ).subcommand(SubCommand::with_name("gettx")
        .about("get tx info")
        .arg(Arg::with_name("tx").index(1).takes_value(true).required(true)
            .help("tx hash"))
    )
}
pub async fn match_command(matches: &ArgMatches<'_>, client: &MetaClient) -> BuckyResult<bool> {
    match matches.subcommand() {
        ("getreceipt", Some(matches)) => {
            let txhash = matches.value_of("tx").expect("must input tx hash");
            // let tx= TxHash::clone_from_slice(&hex::decode(txhash).unwrap());
            match client.get_tx_receipt(&TxId::from_str(txhash)?).await {
                Ok(ret) => {
                    if let Some((receipt, height)) = ret {
                        info!("tx {} on block {},  ret {}", txhash, height, receipt.result);
                        if let Some(address) = receipt.address {
                            info!("contract address: {}", &address);
                        }
                        if let Some(result_value) = receipt.return_value {
                            if let Some(abi_path) = matches.value_of("abi") {
                                let abi = std::fs::read_to_string(abi_path)?;
                                let result_str = ethabi::decode_call_output(&abi, matches.value_of("name").unwrap(), result_value).unwrap();
                                info!("contract return value {}", result_str);
                            } else {
                                info!("contract return value {}", hex::encode(&result_value));
                            }
                        }
                    } else {
                        info!("cannot get receipt for tx {}", txhash);
                    }
                },
                Err(e) => {
                    error!("get receipt err {}", e);
                },
            }
        }
        ("gettx", Some(matches)) => {
            let txhash = matches.value_of("tx").expect("must input tx hash");
            match client.get_tx(&TxId::from_str(txhash)?).await {
                Ok(info) => {
                    info!("tx {} on block {}, ret {}", &info.tx.tx_hash, &info.block_number.unwrap_or_default(), info.tx.result);
                    info!("from {} to {}, value {}", &info.tx.caller, &info.tx.to[0].0, &info.tx.to[0].2);
                },
                Err(e) => {
                    error!("get tx err {}", e);
                }
            }
        }
        ("setconfig", Some(matches)) => {
            let key = matches.value_of("key").expect("must set key");
            let value = matches.value_of("value").expect("must set value");
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            match client.set_config(&caller, key, value, &secret).await {
                Ok(hash) => {
                    info!("set config key {} value {}, hash {}", key, value, hash.to_string());
                },
                Err(e) => {
                    error!("set config failed, err {}", e);
                },
            }
        }
        _ => {return Ok(true)}
    };
    Ok(false)
}

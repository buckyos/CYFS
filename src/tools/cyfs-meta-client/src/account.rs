use clap::{App, SubCommand, Arg, ArgMatches};
use crate::common::{get_caller_arg, get_desc_and_objid_from_matches, get_objid_from_str, DEFAULT_DESC_PATH, get_desc_and_secret_from_matches};
use cyfs_base::{BuckyResult};
use log::*;
use cyfs_meta_lib::{MetaClient};
use cyfs_base_meta::ViewBalanceResult;

pub fn append_command<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
    app.subcommand(SubCommand::with_name("trans")
        .about("trans balance to other account")
        .arg(Arg::with_name("to").short("t").long("to").takes_value(true).required(true)
            .help("to account id or name"))
        .arg(get_caller_arg("from", "f", Some(&DEFAULT_DESC_PATH)))
        .arg(Arg::with_name("balance").index(1).required(true)
            .help("balance amount"))
        .arg(Arg::with_name("coin_id").index(2).default_value("0").help("coin id"))
    )
    .subcommand(SubCommand::with_name("getbalance")
        .about("get balance for account")
        .arg(Arg::with_name("account").index(1).takes_value(true).required(true)
            .help("account id"))
        .arg(Arg::with_name("coin_id").index(2).default_value("0").help("coin id"))
    )
    .subcommand(SubCommand::with_name("setbenefi")
        .about("set benefi for account")
        .arg(Arg::with_name("account").index(1).takes_value(true).required(true)
            .help("account id"))
        .arg(Arg::with_name("benefi").index(2).takes_value(true).required(true)
            .help("benefi id"))
        .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
    )
    .subcommand(SubCommand::with_name("getbenefi")
        .about("get benefi for account")
        .arg(Arg::with_name("account").index(1).takes_value(true).required(true)
            .help("account id"))
    )
}

pub async fn match_command(matches: &ArgMatches<'_>, client: &MetaClient) -> BuckyResult<bool> {
    match matches.subcommand() {
        ("trans", Some(matches)) => {
            let (from, to, secret) = get_desc_and_objid_from_matches(matches, "from", "to", &client).await?;

            let balance = matches.value_of("balance").unwrap().parse::<i64>().map_err(|e|{
                error!("invalid balance amount, err {}", e);
                e
            })?;
            let coin_id = matches.value_of("coin_id").unwrap().parse::<u8>().map_err(|e|{
                error!("invalid coin id, err {}", e);
                e
            })?;

            let hash = client.trans(&from, &to, balance, coin_id, &secret).await?;
            info!("trans {} from {} to {} success, txHash {}", balance, &from.calculate_id(), &to, hash.as_ref().to_string());

        },
        ("getbalance", Some(matches)) => {
            let account = get_objid_from_str(matches.value_of("account").unwrap(), &client).await.map_err(|e|{
                error!("convert account to Objid err, {}", e);
                e
            })?;
            let coin_id = matches.value_of("coin_id").unwrap().parse::<u8>().map_err(|e|{
                error!("invalid coin id, err {}", e);
                e
            })?;

            match client.get_balance(&account, coin_id).await? {
                ViewBalanceResult::Single(s) => {
                    info!("account {} balance {}", &account, s[0].1);
                },
                ViewBalanceResult::Union(u) => {
                    info!("union account {} balance {}, cur seq {}", &account, u[0].1, u[0].2);
                },
            }

        },
        ("setbenefi", Some(matches)) => {
            let account = get_objid_from_str(matches.value_of("account").unwrap(), &client).await.map_err(|e|{
                error!("convert account to Objid err, {}", e);
                e
            })?;
            let benefi = get_objid_from_str(matches.value_of("benefi").unwrap(), &client).await.map_err(|e|{
                error!("convert benefi to Objid err, {}", e);
                e
            })?;

            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let id = client.set_benefi(&account, &benefi, &caller, &secret).await?;
            info!("set address {} benefi to {} success, txHash {}", &account, &benefi, &id);
        }
        ("getbenefi", Some(matches)) => {
            let account = get_objid_from_str(matches.value_of("account").unwrap(), &client).await.map_err(|e|{
                error!("convert account to Objid err, {}", e);
                e
            })?;
            let benefi = client.get_benefi(&account).await?;
            info!("account {} benefi {}", &account, &benefi);

        },
        _ => {return Ok(true)}
    };
    Ok(false)
}

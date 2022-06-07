use clap::{App, ArgMatches, SubCommand, Arg};
use crate::common::{get_caller_arg, get_desc_and_secret_from_matches, DEFAULT_DESC_PATH};
use log::*;
use cyfs_meta_lib::{MetaClient};
use cyfs_base::*;
use cyfs_base_meta::*;
use std::path::Path;
use std::str::FromStr;

pub fn append_command<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {
    app.subcommand(SubCommand::with_name("createunion")
        .about("create an union account and trans balance")
        .arg(Arg::with_name("caller").short("c").long("caller").takes_value(true).required(true)
            .help("caller desc and sec file path, exclude extension, as union left account"))
        .arg(Arg::with_name("union").short("u").long("union").takes_value(true).required(true)
            .help("union account creating infomation file"))
    )
    .subcommand(SubCommand::with_name("createdeviate")
        .about("create a deviate_tx and double sign, push it to chain")
        .arg(Arg::with_name("caller").short("c").long("caller").takes_value(true).required(true)
            .help("caller desc and sec file path, exclude extension, as union left account"))
        .arg(Arg::with_name("deviate").short("d").long("deviate").takes_value(true).required(true)
            .help("deviate tx infomation file"))
    )
    .subcommand(SubCommand::with_name("withdraw")
        .about("withdraw an union account ")
        .arg(get_caller_arg("caller", "c", Some(&DEFAULT_DESC_PATH)))
        .arg(Arg::with_name("union").short("u").long("union").takes_value(true).required(true)
            .help("union account id"))
        .arg(Arg::with_name("amount").index(1).takes_value(true).required(true)
            .help("withdraw amount"))
        .arg(Arg::with_name("coin_id").index(2).default_value("0").help("coin id"))
    )
}

pub async fn match_command(matches: &ArgMatches<'_>, client: &MetaClient) -> BuckyResult<bool> {
    match matches.subcommand() {
        ("createunion", Some(matches)) => {
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let union_path = matches.value_of("union").expect("need union account creating infomation file");
            let (union, _) = CreateUnionTx::decode_from_file(Path::new(union_path), &mut Vec::new())?;
            let hash = client.create_union_account(&caller, union.clone(), &secret).await?;
            info!("create union account id {}, txhash {}", union.body.account.desc().calculate_id(), hash.to_string());
        }
        ("createdeviate", Some(matches)) => {
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let deviate_path = matches.value_of("deviate").expect("need deviate tx infomation file");
            let (deviate, _) = DeviateUnionTx::decode_from_file(Path::new(deviate_path), &mut Vec::new())?;
            let tx = client.create_deviate_tx(&caller, deviate, &secret).await?;
            // tx.add_sign(&right).unwrap();
            info!("create and send deviate tx, hash {}", tx.to_string());
        }
        ("withdraw", Some(matches)) => {
            let (caller, secret) = get_desc_and_secret_from_matches(matches, "caller")?;
            let union_id = matches.value_of("union").expect("need union account id");
            let union_id = ObjectId::from_str(union_id)?;
            let amount = matches.value_of("amount").expect("need amount").parse::<i64>().expect("need number");
            let coin_id = matches.value_of("coin_id").unwrap().parse::<u8>().map_err(|e|{
                error!("invalid coin id, err {}", e);
                e
            })?;
            let hash = client.withdraw_union(&caller, CoinTokenId::Coin(coin_id), &union_id, amount, &secret).await?;
            info!("withdraw union {}, hash {}", union_id.to_string(), hash.to_string());
        }
        _ => {return Ok(true)}
    };
    Ok(false)
}

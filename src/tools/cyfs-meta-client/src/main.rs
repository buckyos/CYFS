mod misc;
mod name;
mod union_account;
mod account;
mod desc;
mod common;
mod contract;

use clap::{App, Arg, AppSettings};
use log::*;
use cyfs_base::BuckyResult;
use cyfs_meta_lib::{MetaMinerTarget, MetaClient};
use std::str::FromStr;

#[async_std::main]
async fn main() ->BuckyResult<()> {
    simple_logger::SimpleLogger::new().with_level(LevelFilter::Debug).init().unwrap();

    let default_target = MetaMinerTarget::default().to_string();
    let meta_arg = Arg::with_name("meta_target").short("m").long("meta_target").takes_value(true)
        .default_value(&default_target)
        .help("meta client target").global(true);

    let mut app = App::new("cyfs-meta-client").version(cyfs_base::get_version()).arg(&meta_arg)
        .setting(AppSettings::GlobalVersion);
    app = crate::account::append_command(app);
    app = crate::desc::append_command(app);
    app = crate::union_account::append_command(app);
    app = crate::name::append_command(app);
    app = crate::misc::append_command(app);
    app = crate::contract::append_command(app);

    let matches = app.get_matches();

    let client = MetaClient::new_target(MetaMinerTarget::from_str(matches.value_of("meta_target").unwrap())?);

    if crate::account::match_command(&matches, &client).await?
        && crate::desc::match_command(&matches, &client).await?
        && crate::union_account::match_command(&matches, &client).await?
        && crate::name::match_command(&matches, &client).await?
        && crate::misc::match_command(&matches, &client).await?
        && crate::contract::match_command(&matches, &client).await?
    {
        error!("unknown command: {}", matches.subcommand().0);
        std::process::exit(1);
    }

    Ok(())
}

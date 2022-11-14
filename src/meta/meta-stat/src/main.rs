mod client;
mod def;
mod storage;
mod notify;
mod reporter;
mod email;
mod sqlite_storage;

use clap::{App, Arg};

use cyfs_base::BuckyResult;
use std::sync::Arc;
use std::path::Path;
use serde::{Deserialize};
use crate::storage::create_storage;
use crate::client::Client;

#[macro_use]
extern crate log;


#[derive(Deserialize)]
pub struct Config {
    db_path: String,
    deadline: u64,
    dingtalk_url: Option<String>,
    email: Option<EmailConfig>,
}

#[derive(Deserialize)]
pub struct EmailConfig {
    email_receiver: String,
    mine_email: String,
    smtp_server: String,
    password: String,
}

#[async_std::main]
async fn main() -> BuckyResult<()> {
    simple_logger::init_with_level(log::Level::Info).unwrap();
    
    let matches = App::new("meta stat").version(cyfs_base::get_version())
        .arg(Arg::with_name("db_path").short("d").long("db_path").value_name("PATH").help("meta archive sqlite db path.\ndefault is current archive_db db path.").takes_value(true))
        .arg(Arg::with_name("last").short("l").long("last").value_name("LAST").help("query last month stat\ndefault is last month.").takes_value(true))
        .arg(Arg::with_name("dingtalk").short("t").long("dingtalk").value_name("DINGTALK").help("dingding talk url").takes_value(true))
        .get_matches(); 

    // 切换目录到当前exe的相对目录
    let root = std::env::current_exe().unwrap();
    let config_path = root.parent().unwrap().join("config.toml");            
    if !config_path.exists() {
        error!("cannot find config file. {}", config_path.display());
        std::process::exit(1);
    }

    let db_path = matches.value_of("db_path").unwrap_or("./");
    let deadline = matches.value_of("last").unwrap_or("1").parse::<u16>().unwrap_or(1);
    let dingtalk = matches.value_of("dingtalk").unwrap_or("https://oapi.dingtalk.com/robot/send?access_token=28788f9229a09bfe8b33e678d4447a2d2d80a334a594e1c942329cab8581f422");
    debug!("db_path: {}, dl: {}, dingtalk: {}", db_path, deadline, dingtalk);

    match toml::from_str::<Config>(std::fs::read_to_string(config_path).unwrap().as_str()) {
        Ok(config) => {
            // 归档按日, 周, 月 统计 sqlite直接对archive_db 数据库表操作
            let storage = Arc::new(create_storage(config.db_path.as_str()).await.map_err(|e|{
                error!("create storage err {}", e);
                e
            })?);
        
            let client = Client::new(&config, storage);
            client.run().await;
        }
        Err(e) => {
            error!("parse config file err {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
    
}

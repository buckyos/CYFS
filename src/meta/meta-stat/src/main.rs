use std::path::Path;
use clap::{App, Arg};

use cyfs_base::{BuckyResult, ObjectTypeCode};
use serde::{Deserialize};
use misc_util::mail::{EmailConfig, send_mail};

#[macro_use]
extern crate log;

#[derive(Deserialize)]
pub struct Config {
    storage: cyfs_meta::stat::StorageConfig,
    email: Option<EmailConfig>,
}

#[async_std::main]
async fn main() -> BuckyResult<()> {
    simple_logger::init_with_level(log::Level::Info).unwrap();
    
    let matches = App::new("meta stat").version(cyfs_base::get_version())
        .arg(Arg::with_name("config").long("config").short("c").takes_value(true).default_value("config.toml").help("meta stat config file path"))
        .arg(Arg::with_name("report").long("report").help("report stat"))
        .arg(Arg::with_name("period").long("period").takes_value(true).default_value("24").help("stat period, hours"))
        .get_matches();

    let config_path = Path::new(matches.value_of("config").unwrap());
    if !config_path.exists() {
        error!("config path {} not exists!", config_path.display());
        std::process::exit(1);
    }

    let period = matches.value_of("period").unwrap().parse::<u16>().unwrap();
    match serde_json::from_slice::<Config>(&std::fs::read(config_path).unwrap()) {
        Ok(config) => {
            // 归档按日, 周, 月 统计 sqlite直接对archive_db 数据库表操作
            let now = chrono::Local::now();
            let from = now - chrono::Duration::hours(period as i64);

            let storage = cyfs_meta::stat::create_storage(Some(config.storage), true);
            let stat = storage.get_stat(from.with_timezone(&chrono::Utc)).await?;
            let total_people = storage.get_desc_total(Some(ObjectTypeCode::People)).await?;
            let total_device = storage.get_desc_total(Some(ObjectTypeCode::Device)).await?;
            let mut output = String::new();
            output += &format!("Meta Stat from {} to {}\n", from.format("%F %T"), now.format("%F %T"));
            output += &format!("-----------------------------------\n");
            output += &format!("total People: {}\n", total_people);
            output += &format!("total Device: {}\n", total_device);
            output += &format!("new People: {}\n", stat.new_people);
            output += &format!("new Device: {}\n", stat.new_device);
            output += &format!("active People: {}\n", stat.active_people.len());
            output += &format!("active Device: {}\n", stat.active_device.len());
            output += &format!("api call failed:\n");
            for (name, num) in &stat.api_fail {
                output += &format!("\t{}, \t{} times\n", name, num);
            }
            output += &format!("api call success:\n");
            for (name, num) in &stat.api_success {
                output += &format!("\t{}, \t{} times\n", name, num);
            }

            println!("{}", &output);

            if matches.is_present("report") {
                println!("reporting...");
                if let Some(config) = config.email {
                    let subject = format!("{} Meta Chain Stat {}", cyfs_base::get_channel().to_string(), chrono::Local::today().format("%F"));
                    let _ = send_mail(config, subject, output).await.map_err(|e| {
                        error!("send mail err {}", e);
                        e
                    });
                }

            }
        }
        Err(e) => {
            error!("parse config file err {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
    
}

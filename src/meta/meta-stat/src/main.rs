use clap::{App, Arg};

#[macro_use]
extern crate log;

#[async_std::main]
async fn main() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();
    
    let matches = App::new("meta stat").version(cyfs_base::get_version())
        .arg(Arg::with_name("db_path").short("d").long("db_path").value_name("PATH").help("meta archive sqlite db path.\ndefault is current archive_db db path.").takes_value(true))
        .get_matches();

    let db_path = matches.value_of("db_path").unwrap_or("./archive_db");
    info!("db_path: {}", db_path);

    //TODO: 归档按日, 周, 月 统计 sqlite直接对archive_db 数据库表操作
}
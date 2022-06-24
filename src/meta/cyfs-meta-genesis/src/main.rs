use std::path::{Path};
use commandlines;
use cyfs_meta::*;
use simple_logger::SimpleLogger;

fn main() {
    SimpleLogger::new().with_level(log::LevelFilter::Debug).init().unwrap();
    let c = commandlines::Command::new();
    if let Some(config_path) = c.get_definition_for("--config") {
        if ChainCreator::create_chain(Path::new(config_path.as_ref()), Path::new(config_path.as_ref()).parent().unwrap(), new_sql_storage).is_ok() {
            println!("create chain ok");
        } else {
            println!("create chain failed");
        }
    } else {
        println!("cyfs-meta-genesis --config=[config path]");
    }
}

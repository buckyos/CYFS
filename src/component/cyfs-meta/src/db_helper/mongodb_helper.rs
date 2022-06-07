use cyfs_base::*;
use mongodb::options::{ClientOptions, ServerAddress};
use std::sync::RwLock;
use lazy_static::lazy_static;
use mongodb::Client;
use crate::*;

struct MongoDbClient {
}

lazy_static! {
    static ref MONGODB_CLIENT: RwLock<Option<mongodb::Client>> = RwLock::new(None);
}

impl MongoDbClient {
    pub fn init(hostname: &str, port: u16) -> BuckyResult<()> {
        let options = ClientOptions::builder().hosts(vec![ServerAddress::Tcp { host: hostname.to_string(), port: Some(port) }]).build();
        let mut client = MONGODB_CLIENT.write().unwrap();
        *client = Some(Client::with_options(options).map_err(|e| {
            log::error!("open {} {} failed. {:?}", hostname, port, e);
            crate::meta_err!(ERROR_EXCEPTION)
        })?);
        Ok(())
    }
}

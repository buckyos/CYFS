use clap::{App, Arg};
use cyfs_meta::*;
use std::sync::Arc;
use std::path::Path;

#[async_std::main]
async fn main() {
    cyfs_debug::CyfsLoggerBuilder::new_service("meta_miner")
        .build()
        .unwrap()
        .start();

    cyfs_debug::PanicBuilder::new("app", "meta_miner")
        .build()
        .start();

    let matches = App::new("cyfs meta miner").version(cyfs_base::get_version())
        .arg(Arg::with_name("path").short("p").long("path").value_name("PATH").help("set meta chain path.\ndefault is current path.").takes_value(true))
        .arg(Arg::with_name("port").long("port").help("set meta chain service port.\ndefault is 1423.").takes_value(true))
        .arg(Arg::with_name("trace").long("trace").help("set meta chain trace\ndefault is trace.").takes_value(true))
        .get_matches();

    let chain_path = matches.value_of("path").unwrap_or("./");
    let chain_port = matches.value_of("port").unwrap_or(::cyfs_base::CYFS_META_MINER_PORT.to_string().as_str()).parse::<u16>().unwrap_or(::cyfs_base::CYFS_META_MINER_PORT);
    let trace = matches.value_of("trace").unwrap_or("true").parse::<bool>().unwrap_or(true);
    
    let miner: Arc<dyn Miner> = ChainCreator::start_miner_instance(Path::new(chain_path), new_sql_storage, trace, new_archive_storage).unwrap();
    let server = MetaHttpServer::new(miner, chain_port);
    server.run().await.unwrap();
}

#[cfg(test)]
mod main_tests {
    use cyfs_base::*;
    use cyfs_meta_lib::{MetaClient, MetaClientHelper, MetaMinerTarget};
    use std::str::FromStr;

    pub const OBJ_ID: &'static str = "5r4MYfFU8UfvqjN3FPxcf7Ta4gx9c56jrXdNwnVyp29e";
    //cargo test -- --nocapture
    #[async_std::test]
    async fn test_get_meta_object() -> BuckyResult<()> {
        let meta_client = MetaClient::new_target(MetaMinerTarget::from_str("http://127.0.0.1:1423")?)
        .with_timeout(std::time::Duration::from_secs(60 * 2));

        let object_id = ObjectId::from_str(OBJ_ID).unwrap();
        let ret = MetaClientHelper::get_object(&meta_client, &object_id).await?;
        if ret.is_none() {
            let msg = format!(
                "load object from meta chain but not found! id={}",
                object_id
            );
            println!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let (_, object_raw) = ret.unwrap();

        // 解码
        let p = People::clone_from_slice(&object_raw).map_err(|e| {
            let msg = format!("decode people object failed! id={}, {}", object_id, e);
            println!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;
        println!("people: {:?}, {:?}", p.name(), p.ood_list());

        Ok(())
    }
}
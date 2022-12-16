use std::path::Path;
use std::str::FromStr;
use std::time::Duration;
use clap::Arg;
use log::LevelFilter;
use cyfs_base::{BuckyErrorCode, BuckyResult, CYFS_SN_NAME, NamedObject, NameLink, NameState, ObjectDesc, RawConvertTo, TxId};
use cyfs_base_meta::{Data, SavedMetaObject};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};

async fn wait_tx(client: &MetaClient, tx: &TxId) -> BuckyResult<bool> {
    for _ in 0..4 {
        async_std::task::sleep(Duration::from_secs(10)).await;
        let receipt = client.get_tx_receipt(&tx).await;
        match receipt {
            Ok(Some((receipt, height))) => {
                println!("tx {} land on metachain height {}, result {}", tx, height, receipt.result);
                return Ok(receipt.result == 0);
            }
            Err(e) => {
                if e.code() != BuckyErrorCode::NotFound {
                    return Err(e)
                }
            }
            _ => {}
        }
    }

    Ok(false)
}

async fn run() {
    let default_target = MetaMinerTarget::default().to_string();
    let matches = clap::App::new("sn-updater")
        .version(cyfs_base::get_version())
        .about("update sn list to meta chain")
        .arg(Arg::with_name("target").short("t").long("target").takes_value(true).default_value(&default_target))
        .arg(Arg::with_name("sndir").short("d").long("sn-dir").required(true).takes_value(true).help("sn desc dir"))
        .arg(Arg::with_name("snname").short("n").long("sn-name").takes_value(true).default_value(CYFS_SN_NAME).help("sn name on chain"))
        .arg(Arg::with_name("owner").short("o").long("owner").required(true).takes_value(true).help("owner desc/sec path, exclude ext"))
        .get_matches();
    let owner_path = Path::new(matches.value_of("owner").unwrap());
    let sn_path = Path::new(matches.value_of("sndir").unwrap());
    let (owner, owner_key) = cyfs_util::get_desc_from_file(&owner_path.with_extension("desc"), &owner_path.with_extension("sec")).unwrap();
    let owner_id = owner.calculate_id();
    let meta_client = MetaClient::new_target(MetaMinerTarget::from_str(matches.value_of("target").unwrap()).unwrap());

    let sn_name = matches.value_of("snname").unwrap();
    let mut name_info = meta_client.get_name(sn_name).await.unwrap();
    if name_info.is_none() {
        println!("name is not exist, buy it");

        let buy_tx = meta_client.bid_name(&owner, None, sn_name, 0, 0, &owner_key).await.unwrap();
        let ret = wait_tx(&meta_client, &buy_tx).await.unwrap();
        if !ret {
            println!("ERROR: buy sn name failed!");
            return;
        }

        name_info = meta_client.get_name(sn_name).await.unwrap();
    }

    loop {
        println!("checking name {} status", sn_name);
        if name_info.as_ref().unwrap().1 == NameState::Normal {
            println!("name {} status normal.", sn_name);
            break;
        }

        println!("name {} status {}, wait status normal", sn_name, name_info.unwrap().1 as u8);
        async_std::task::sleep(Duration::from_secs(10)).await;
        name_info = meta_client.get_name(sn_name).await.unwrap();
    }

    let sn = cyfs_util::SNDirGenerator::gen_from_dir(&Some(owner_id), sn_path).unwrap();

    let desc_tx = meta_client.create_desc(&owner, &SavedMetaObject::Data(Data { id: sn.desc().calculate_id(), data: sn.to_vec().unwrap() }), 0, 0, 0, &owner_key).await.unwrap();

    println!("wait sn desc put on meta...");
    let ret = wait_tx(&meta_client, &desc_tx).await.unwrap();
    if !ret {
        println!("ERROR: sn desc put failed!");
        return;
    }

    name_info.as_mut().unwrap().0.record.link = NameLink::ObjectLink(sn.desc().calculate_id());

    let name_tx = meta_client.update_name(&owner, sn_name, name_info.unwrap().0, 0, &owner_key).await.unwrap();
    println!("wait name update on meta...");
    let ret = wait_tx(&meta_client, &name_tx).await.unwrap();
    if !ret {
        println!("ERROR: name update failed!");
        return;
    }

}

fn main() {
    simple_logger::SimpleLogger::new().with_level(LevelFilter::Debug).init().unwrap();
    async_std::task::block_on(async {
        run().await
    });
}

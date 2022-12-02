mod local_storage;
mod manager;
mod runner;
mod state;
mod factor;

#[macro_use]
extern crate log;

use cyfs_base::*;
use cyfs_debug::*;
use local_storage::*;
use manager::*;
use runner::*;
use state::*;

use clap::{App, Arg};
use std::str::FromStr;
use std::sync::Arc;
use std::path::PathBuf;

async fn run(
    object_id: ObjectId,
    private_key: Vec<PrivateKey>,
    difficulty: u8,
    threads: u32,
) -> BuckyResult<()> {
    let state_storage = PowStateLocalStorage::new_default();
    let state_storage = Arc::new(Box::new(state_storage) as Box<dyn PoWStateStorage>);

    let id_range = 0..1024;

    let manager = PoWStateManager::load_or_new(
        object_id,
        private_key,
        difficulty,
        id_range,
        state_storage.clone(),
    )
    .await?;
    if manager.check_complete() {
        let state = manager.state();
        let msg = format!(
            "PoW finished without result! object={}, difficulty={}",
            state.data.object_id, state.data.difficulty
        );

        println!("{}", msg);
        info!("{}", msg);

        return Ok(());
    }

    manager.start_save();

    let sync = Arc::new(Box::new(manager) as Box<dyn PoWThreadStateSync>);
    let state = sync.state();

    let runner = PowRunner::new(sync.clone());
    runner.run(state.data.difficulty, threads)?;

    let state = sync.state();
    state_storage.save(&state).await?;

    let msg = if let Some(nonce) = state.data.nonce {
        format!(
            "PoW got result! object={}, difficulty={}, nonce={}",
            state.data.object_id, state.data.difficulty, nonce
        )
    } else {
        format!(
            "PoW finished without result! object={}, difficulty={}",
            state.data.object_id, state.data.difficulty
        )
    };

    println!("{}", msg);
    info!("{}", msg);

    Ok(())
}

fn main() {
    let app = App::new("cyfs-pow")
        .version(cyfs_base::get_version())
        .about("object pow tool for cyfs system")
        .author("CYFS <dev@cyfs.com>")
        .arg(
            Arg::with_name("object")
                .short("o")
                .long("object")
                .takes_value(true)
                .help("The object id used to calculate the difficulty"),
        )
        .arg(
            Arg::with_name("threads")
                .short("t")
                .long("threads")
                .takes_value(true)
                .help("Threads count, default is 1, set to 0 will use as many as cpu nums"),
        )
        .arg(
            Arg::with_name("difficulty")
                .short("d")
                .long("diff")
                .takes_value(true)
                //.required(true)
                .help("Target difficulty, value range 0-255"),
        )
        .arg(
            Arg::with_name("private-key")
                .short("p")
                .long("private-key")
                .takes_value(true)
                //.required(true)
                .help("Private key, support multiple, separated by semicolon"),
        ).arg(
            Arg::with_name("factor")
                .short("f")
                .long("factor")
                .takes_value(false)
                .required(false)
                .help("Run private key factor test"),
        );

    let matches = app.get_matches();

    if matches.is_present("factor") {
        factor::run();
        std::process::exit(0);
    }

    let obj = match matches.value_of("object") {
        Some(v) => match ObjectId::from_str(v) {
            Ok(id) => id,
            Err(e) => {
                println!("invalid object param value: {}, {}", v, e);
                std::process::exit(-1);
            }
        },
        None => {
            println!("object param missing!");
            std::process::exit(-1);
        }
    };

    let mut threads: u32 = match matches.value_of("threads") {
        Some(v) => v
            .parse()
            .map_err(|e| {
                println!("invalid threads param value: {}", e);
                std::process::exit(-1);
            })
            .unwrap(),
        None => 1,
    };

    if threads <= 0 {
        threads = num_cpus::get() as u32;
    }

    let difficulty: u8 = match matches.value_of("difficulty") {
        Some(v) => v
            .parse()
            .map_err(|e| {
                println!("invalid difficulty param value: {}", e);
                std::process::exit(-1);
            })
            .unwrap(),
        None => {
            println!("difficulty param is missing!");
            std::process::exit(-1);
        }
    };

    let private_key = match matches.value_of("private-key") {
        Some(v) => {
            let list: Vec<&str> = v.split(";").collect();
            let mut sk_list = vec![];
            for s in list {
                let mut buf = vec![];
                let path: PathBuf = s.into();
                let (sk, _) = PrivateKey::decode_from_file(&path, &mut buf)
                    .map_err(|e| {
                        println!("load private key from file failed: {}, {}", s, e);
                        std::process::exit(-1);
                    })
                    .unwrap();
                println!(
                    "load private key from file! {}, type={:?}",
                    s,
                    sk.key_type()
                );
                sk_list.push(sk);
            }

            if sk_list.is_empty() {
                println!("private key list is empty!");
                std::process::exit(-1);
            }

            sk_list
        }
        None => {
            println!("private key param is missing!");
            std::process::exit(-1);
        }
    };

    CyfsLoggerBuilder::new_app("cyfs-pow")
        .level("debug")
        .console("debug")
        .enable_bdt(Some("warn"), Some("warn"))
        .build()
        .unwrap()
        .start();

    PanicBuilder::new("tools", "cyfs-pow")
        .exit_on_panic(true)
        .build()
        .start();

    async_std::task::block_on(async move {
        match run(obj, private_key, difficulty, threads).await {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                println!("Error occured during calc difficulty! {}", e);
                std::process::exit(-1);
            }
        }
    });
}

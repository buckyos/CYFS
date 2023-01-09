mod util;
mod modify;
mod sign;

use clap::{SubCommand, App, Arg};
use crate::show::{show_desc, show_desc_subcommand};
use crate::create::{create_desc, create_subcommand};
use crate::modify::{modify_subcommand, modify_desc};
use log::*;
use cyfs_base::{StandardObject, FileDecoder, BuckyError, BuckyErrorCode};
use crate::sign::{sign_subcommand, sign_desc};

pub mod desc;
mod show;
mod create;

fn calc_nonce(peer_desc_file: &str, _bits: u32) {
    // let mut nonce = [0u8; NONCE_LENGTH];

    let calc_ret = match StandardObject::decode_from_file(peer_desc_file.as_ref(), &mut vec![]) {
        Ok((desc, _)) => {
            match desc {
                StandardObject::Device(_p) => {
                    // TODO: 加calc pow
                    // p.const_info.calc_pow(bits, &mut nonce)
                    Ok(())
                },
                // StandardObject::SimpleGroup(_p) => {
                //     // TODO: 加calc pow
                //     // p.const_info.calc_pow(bits, &mut nonce)
                //     Ok(())
                // },
                _ => {
                    error!("not support object type");
                    Err(BuckyError::new(BuckyErrorCode::NotSupport, ""))
                }
            }
        },
        Err(e) => {
            error!("decode peer desc failed, err {}", e);
            Err(e)
        },
    };
    match calc_ret {
        Ok(_) => {
            /*
            for i in 0..nonce.len() {
                print!("{:02x}", nonce[i]);
            }
             */
            info!("calc nonce finish");
        }
        Err(e) => {
            error!("calc nonce failed, err {}", e);
        }
    }
}

fn calc_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("calc").about("Calc object nonce")
        .arg(Arg::with_name("bits").long("bits").short("b").required(true).takes_value(true)
            .help("Target bits to calc nonce"))
        .arg(Arg::with_name("file").index(1).required(true).takes_value(true)
            .help("Input Object info file"))
}


async fn main_run() {
    simple_logger::SimpleLogger::new().with_level(LevelFilter::Debug).init().unwrap();
    let matches = App::new("desc-tool").version(cyfs_base::get_version()).about("tool to create or show desc files")
        .subcommand(create_subcommand())
        .subcommand(show_desc_subcommand())
        .subcommand(calc_subcommand())
        .subcommand(modify_subcommand())
        .subcommand(sign_subcommand())
        .get_matches();

    match matches.subcommand() {
        ("create", Some(matches)) => {
            create_desc(matches).await;
        },
        ("show", Some(matches)) => {
            show_desc(matches);
        },
        ("calc", Some(matches)) => {
            let bits = matches.value_of("bits").unwrap().parse::<u32>().unwrap();
            calc_nonce(matches.value_of("file").unwrap(), bits);
        },
        ("modify", Some(matches)) => {
            modify_desc(matches);
        },
        ("sign", Some(matches)) => {
            sign_desc(matches).await;
        }
        v @ _ => {
            error!("unknown command: {}", v.0);
            std::process::exit(1);
        }
    }
}

fn main() {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run());
}
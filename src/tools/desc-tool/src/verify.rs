use clap::{App, SubCommand, Arg, ArgMatches};
use cyfs_base::{AnyNamedObject, FileDecoder, StandardObject, NamedObject, SingleKeyObjectDesc, RsaCPUObjectVerifier};
use log::*;

pub fn sign_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("verify").about("verify desc")
        .arg(Arg::with_name("desc").index(1).required(true).help("desc file to sign"))
        .arg(Arg::with_name("pubkey").short("p").long("pubkey").takes_value(true).help("desc file include pubkey"))
}

pub async fn verify_desc(matches: &ArgMatches<'_>) {
    let (desc, _) = AnyNamedObject::decode_from_file(matches.value_of("desc").unwrap().as_ref(), &mut vec![]).unwrap();
    let (pubkey_desc, _) = AnyNamedObject::decode_from_file(matches.value_of("pubkey").unwrap().as_ref(), &mut vec![]).unwrap();

    let pubkey = match &pubkey_desc {
        AnyNamedObject::Standard(o) => {
            match o {
                StandardObject::Device(o) => {Some(o.desc().public_key())},
                StandardObject::People(o) => {Some(o.desc().public_key())},
                _ => None,
            }
        },
        AnyNamedObject::Core(_) => {None},
        AnyNamedObject::DECApp(_) => {None},
    };
    if let Some(pubkey) = pubkey {
        let verifier = RsaCPUObjectVerifier::new(pubkey.clone());
        // verifier.verify()
    } else {
        error!("pubkey desc not include pubkey")
    }
}
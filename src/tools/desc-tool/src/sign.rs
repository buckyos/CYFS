use clap::{App, SubCommand, Arg, ArgMatches};

use cyfs_base::{PrivateKey, FileDecoder, AnyNamedObject, RsaCPUObjectSigner, SignatureSource, ObjectId, StandardObject, FileEncoder, ObjectLink, SIGNATURE_SOURCE_REFINDEX_OWNER, SIGNATURE_SOURCE_REFINDEX_SELF};
use cyfs_base::{sign_and_push_named_object_desc, sign_and_push_named_object_body, sign_and_set_named_object_body, sign_and_set_named_object_desc};

use log::*;
use std::str::FromStr;

pub fn sign_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("sign").about("sign desc")
        .arg(Arg::with_name("desc").takes_value(true).index(1).required(true)
            .help("desc file to sign"))
        .arg(Arg::with_name("secret").takes_value(true).short("s").long("secret").required(true)
            .help("secret file for sign"))
        .arg(Arg::with_name("sign_desc").short("d").long("sign_desc")
            .help("add sign for desc"))
        .arg(Arg::with_name("sign_body").short("b").long("sign_body")
            .help("add sign for body"))
        .arg(Arg::with_name("sign_source").short("t").long("sign_source").takes_value(true)
            .help("sign type, default single"))
        .arg(Arg::with_name("append").short("a").long("append")
            .help("append sign, otherwise replace all signs"))
}

macro_rules! match_any_obj_mut {
    ($on:ident, $o:ident, $body:tt, $chunk_id:ident, $chunk_body:tt) => {
        match &mut $on {
            AnyNamedObject::Standard(o) => {
                match o {
                    StandardObject::Device($o) => {$body},
                    StandardObject::People($o) => {$body},
                    StandardObject::SimpleGroup($o) => {$body},
                    StandardObject::Contract($o) => {$body},
                    StandardObject::UnionAccount($o) => {$body},
                    StandardObject::ChunkId($chunk_id) => {$chunk_body},
                    StandardObject::File($o) => {$body},
                    StandardObject::Org($o) => {$body},
                    StandardObject::AppGroup($o) => {$body},
                    StandardObject::Dir($o) => {$body},
                    StandardObject::Diff($o) => {$body},
                    StandardObject::ProofOfService($o) => {$body},
                    StandardObject::Tx($o) => {$body},
                    StandardObject::Action($o) => {$body},
                    StandardObject::ObjectMap($o) => {$body},
                }
            },
            AnyNamedObject::Core($o) => {
                $body
            },
            AnyNamedObject::DECApp($o) => {
                $body
            },
        }
    }
}

pub async fn sign_desc(matches: &ArgMatches<'_>) {
    if let Ok((private, _)) = PrivateKey::decode_from_file(matches.value_of("secret").unwrap().as_ref(), &mut vec![]) {
        if let Ok((mut obj, _)) = AnyNamedObject::decode_from_file(matches.value_of("desc").unwrap().as_ref(), &mut vec![]) {
            let signer = RsaCPUObjectSigner::new(private.public(), private);
            let sign_type = matches.value_of("sign_source").map(|str|{
                match str {
                    "self" => SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF),
                    "owner" => SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER),
                    str @ _ => {
                        if let Ok(obj) = ObjectId::from_str(str) {
                            SignatureSource::Object(ObjectLink { obj_id: obj, obj_owner: None })
                        } else if let Ok(index) = str.parse::<u8>() {
                            SignatureSource::RefIndex(index)
                        } else {
                            SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER)
                        }
                    }
                }

            }).unwrap_or(SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER));
            let mut signed = false;
            let append = matches.is_present("append");
            if matches.is_present("sign_desc") {
                if append {
                    match_any_obj_mut!(obj, o, {sign_and_push_named_object_desc(&signer, o, &sign_type).await.unwrap(); signed = true;}, _id, {error!("not support sign for chunkid");});
                } else {
                    match_any_obj_mut!(obj, o, {sign_and_set_named_object_desc(&signer, o, &sign_type).await.unwrap(); signed = true;}, _id, {error!("not support sign for chunkid");});
                }

            };
            if matches.is_present("sign_body") {
                if append {
                    match_any_obj_mut!(obj, o, {sign_and_push_named_object_body(&signer, o, &sign_type).await.unwrap(); signed = true;}, _id, {error!("not support sign for chunkid");});
                } else {
                    match_any_obj_mut!(obj, o, {sign_and_set_named_object_body(&signer, o, &sign_type).await.unwrap(); signed = true;}, _id, {error!("not support sign for chunkid");});
                }
            }
            if signed {
                obj.encode_to_file(matches.value_of("desc").unwrap().as_ref(), true).unwrap();
            }
        } else {
            error!("invalid desc file");
        }
    } else {
        error!("invalid secret!");
    }
}

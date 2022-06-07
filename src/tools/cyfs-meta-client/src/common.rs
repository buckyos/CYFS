use clap::{ArgMatches, Arg};
use std::path::Path;
use async_recursion::async_recursion;
use log::*;
use lazy_static::lazy_static;
use cyfs_base::{BuckyResult, ObjectId, NameLink, BuckyError, BuckyErrorCode, PrivateKey, StandardObject};
use cyfs_meta_lib::MetaClient;
use std::str::FromStr;

#[async_recursion]
pub async fn get_objid_from_str(str: &str, client: &MetaClient) -> BuckyResult<ObjectId> {
    return if let Ok(id) = ObjectId::from_str(str) {
        Ok(id)
    } else {
        warn!("str {} is not a valid id, try search as name", str);
        if let Ok(Some((info, _))) = client.get_name(str).await {
            match info.record.link {
                NameLink::ObjectLink(obj) => { Ok(obj) },
                NameLink::OtherNameLink(name) => {
                    get_objid_from_str(&name, client).await
                },
                NameLink::IPLink(_) => {
                    Err(BuckyError::new(BuckyErrorCode::NotFound, ""))
                },
            }
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, ""))
        }
    }
}

pub fn get_desc_and_secret_from_matches(matches: &ArgMatches<'_>, desc_param: &str) -> BuckyResult<(StandardObject, PrivateKey)> {
    let desc_path = Path::new(matches.value_of(desc_param).expect("must set caller desc/sec file"));
    let desc = cyfs_util::get_desc_from_file(&desc_path.with_extension("desc"), &desc_path.with_extension("sec"))?;
    Ok(desc)
}

pub async fn get_desc_and_objid_from_matches(matches: &ArgMatches<'_>, desc_param: &str, obj_param: &str, client: &MetaClient) -> BuckyResult<(StandardObject, ObjectId, PrivateKey)> {
    let (desc, secret) = get_desc_and_secret_from_matches(matches, desc_param)?;
    let obj = get_objid_from_str(matches.value_of(obj_param).unwrap(), &client).await.map_err(|e|{
        error!("convert to param to Objid err, {}", e);
        e
    })?;
    Ok((desc, obj, secret))
}

pub fn get_caller_arg<'a, 'b>(name: &'a str, short: &'a str, default: Option<&'a str>) -> Arg<'a, 'b> {
    let mut arg = Arg::with_name(name).short(short).long(name).takes_value(true).required(true)
        .help("desc and sec file path, exclude extension");
    if let Some(default) = default {
        arg = arg.default_value(default);
    }
    arg
}

lazy_static! {
    pub static ref DEFAULT_DESC_PATH: String = dirs::home_dir().map_or("".to_owned(), |home|{
        home.join(".cyfs").join("owner").to_str().unwrap().to_owned()
    });
}



/*
pub fn append_command<'a, 'b>(app: App<'a, 'b>) -> App<'a, 'b> {}
pub fn async match_command(matches: &'_ ArgMatches) -> BuckyResult<bool> {
    match matches.subcommand() {
        _ => {return Ok(true)}
    };
    Ok(false)
}
 */

use clap::{App, Arg, ArgMatches, SubCommand};
use cyfs_base::{
    BodyContent, BuckyError, BuckyErrorCode, BuckyResult, FileDecoder, FileEncoder, NamedObject,
    ObjectDesc, ObjectId, ObjectType, OwnerObjectDesc, PrivateKey, RawConvertTo, RawEncode,
    RawFrom, StandardObject,
};
use cyfs_base_meta::{Data, SavedMetaObject};
use cyfs_core::{AppList, AppListObj, AppStatus, AppStatusObj, DecApp, DecAppId, DecAppObj};
use cyfs_lib::{
    NONAPILevel, NONObjectInfo, NONOutputRequestCommon, NONPutObjectRequest, SharedCyfsStack,
};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use lazy_static::lazy_static;
use log::*;
use std::path::Path;
use std::str::FromStr;

// put: 运行在一个跨进程non stack后边，通过PutApp对象添加/改变一个App状态
// remove: 运行在一个跨进程non stack后边，通过RemoveApp对象移除一个App
// app create: 创建一个App对象，通过参数指定是否上链
// app set: 给App对象set一个source，App对象可以是文件也可以是链上ID
// app remove: 移除一个App对象的指定source
// app clean: 移除一个App对象的所有source
// app show: 展示一个App对象的内容
// list create: 创建一个空AppList对象，通过参数指定是否上链
// list put: 给AppList对象put一个App，对象可以是文件也可以是链上ID
// list remove: 给AppList对象移除一个App，对象可以是文件也可以是链上ID
// list clean: 清除AppList
// list show: 清空AppList对象，对象可以是文件也可以是链上ID
// update：通过app_config.json文件自动修改链上App对象，也修改对应AppList，兼容现在ci app pub输出的json格式

fn add_id_or_file_arg<'a, 'b>(cmd: App<'a, 'b>) -> App<'a, 'b> {
    cmd.arg(
        Arg::with_name("id")
            .index(1)
            .takes_value(true)
            .help("obj id, must on chain"),
    )
    .arg(
        Arg::with_name("file")
            .short("f")
            .long("file")
            .takes_value(true)
            .conflicts_with("id")
            .help("object file path"),
    )
    .arg(
        Arg::with_name("owner")
            .short("o")
            .long("owner")
            .takes_value(true)
            .help("app owner desc/sec file"),
    )
    .arg(
        Arg::with_name("name")
            .short("n")
            .long("name")
            .default_value("")
            .help("app or app list name"),
    )
}

async fn put_object<D, T, N>(stack: &SharedCyfsStack, obj: &N)
where
    D: ObjectType,
    T: RawEncode,
    N: RawConvertTo<T>,
    N: NamedObject<D>,
    <D as ObjectType>::ContentType: BodyContent,
{
    let object_id = obj.desc().calculate_id();
    match stack
        .non_service()
        .put_object(NONPutObjectRequest {
            common: NONOutputRequestCommon::new(NONAPILevel::Router),
            object: NONObjectInfo::new_from_object_raw(obj.to_vec().unwrap()).unwrap(),
        })
        .await
    {
        Ok(_) => {
            info!("put obj [{}] to ood success!", &object_id);
        }
        Err(e) => {
            if e.code() != BuckyErrorCode::Ignored {
                error!("put obj [{}] to ood failed! {}", &object_id, e);
            } else {
                info!("put obj [{}] to ood success!", &object_id);
            }
        }
    }
}

async fn put_object_ex<D, T, N>(obj: &N)
where
    D: ObjectType,
    T: RawEncode,
    N: RawConvertTo<T>,
    N: NamedObject<D>,
    <D as ObjectType>::ContentType: BodyContent,
{
    let cyfs_stack = match SharedCyfsStack::open_default(None).await {
        Ok(stack) => Some(stack),
        Err(e) => {
            warn!("cannot open local stack, err {}", e);
            None
        }
    };

    if let Some(stack) = cyfs_stack {
        put_object(&stack, obj).await;
    }
}

fn get_owner(matches: &ArgMatches<'_>) -> Option<(StandardObject, PrivateKey)> {
    if let Some(owner_path) = matches.value_of("owner") {
        let ret = cyfs_util::get_desc_from_file(
            &Path::new(owner_path).with_extension("desc"),
            &Path::new(owner_path).with_extension("sec"),
        )
        .unwrap();
        Some(ret)
    } else {
        None
    }
}

async fn get_list(matches: &ArgMatches<'_>) -> BuckyResult<(AppList, SaveTarget)> {
    if let Some(id_str) = matches.value_of("id") {
        let meta_client = MetaClient::new_target(
            MetaMinerTarget::from_str(matches.value_of("meta_target").unwrap()).unwrap(),
        );

        let id = ObjectId::from_str(id_str).unwrap();

        match meta_client.get_desc(&id).await {
            Ok(data) => {
                if let SavedMetaObject::Data(data) = data {
                    let list = AppList::clone_from_slice(&data.data).unwrap();
                    let owner = get_owner(matches);
                    Ok((list, SaveTarget::Meta(meta_client, owner)))
                } else {
                    error!("get {} from meta failed, unmatch", &id);
                    Err(BuckyError::from(BuckyErrorCode::NotMatch))
                }
            }
            Err(e) => {
                error!("get {} from meta failed, err {}", &id, e);
                Err(e)
            }
        }
    } else if let Some(file_path) = matches.value_of("file") {
        let (list, _) = AppList::decode_from_file(file_path.as_ref(), &mut vec![])?;
        let owner = get_owner(matches);
        Ok((list, SaveTarget::File(file_path.to_owned(), owner)))
    } else {
        let meta_client = MetaClient::new_target(
            MetaMinerTarget::from_str(matches.value_of("meta_target").unwrap()).unwrap(),
        );
        let owner = get_owner(matches);
        if owner.is_none() {
            error!("must use owner and name when no id or file!");
            return Err(BuckyError::from(BuckyErrorCode::InvalidInput));
        }
        let list_id = AppList::generate_id(
            owner.as_ref().unwrap().0.calculate_id(),
            matches.value_of("name").unwrap(),
            matches.value_of("type").unwrap(),
        );

        match meta_client.get_desc(&list_id).await {
            Ok(data) => {
                if let SavedMetaObject::Data(data) = data {
                    let list = AppList::clone_from_slice(&data.data).unwrap();
                    Ok((list, SaveTarget::Meta(meta_client, owner)))
                } else {
                    error!("get {} from meta failed, unmatch", &list_id);
                    Err(BuckyError::from(BuckyErrorCode::NotMatch))
                }
            }
            Err(e) => {
                error!("get {} from meta failed, err {}", &list_id, e);
                Err(e)
            }
        }
    }
}

async fn get_app(matches: &ArgMatches<'_>) -> BuckyResult<(DecApp, SaveTarget)> {
    if let Some(id_str) = matches.value_of("id") {
        let meta_client = MetaClient::new_target(
            MetaMinerTarget::from_str(matches.value_of("meta_target").unwrap()).unwrap(),
        );
        let id = ObjectId::from_str(id_str).unwrap();

        return match meta_client.get_desc(&id).await {
            Ok(data) => {
                if let SavedMetaObject::Data(data) = data {
                    let app = DecApp::clone_from_slice(&data.data).unwrap();
                    let owner = get_owner(matches);
                    Ok((app, SaveTarget::Meta(meta_client, owner)))
                } else {
                    error!("get {} from meta failed, unmatch", &id);
                    Err(BuckyError::from(BuckyErrorCode::NotMatch))
                }
            }
            Err(e) => {
                error!("get {} from meta failed, err {}", &id, e);
                Err(e)
            }
        };
    } else if let Some(file_path) = matches.value_of("file") {
        let (app, _) = DecApp::decode_from_file(file_path.as_ref(), &mut vec![])?;
        let owner = get_owner(matches);
        return Ok((app, SaveTarget::File(file_path.to_owned(), owner)));
    } else {
        let meta_client = MetaClient::new_target(
            MetaMinerTarget::from_str(matches.value_of("meta_target").unwrap()).unwrap(),
        );
        let owner = get_owner(matches);
        if owner.is_none() {
            error!("must use owner and name when no id or file!");
            return Err(BuckyError::from(BuckyErrorCode::InvalidInput));
        }
        let id = DecApp::generate_id(
            owner.as_ref().unwrap().0.calculate_id(),
            matches.value_of("name").unwrap(),
        );

        match meta_client.get_desc(&id).await {
            Ok(data) => {
                if let SavedMetaObject::Data(data) = data {
                    let app = DecApp::clone_from_slice(&data.data).unwrap();
                    Ok((app, SaveTarget::Meta(meta_client, owner)))
                } else {
                    error!("get {} from meta failed, unmatch", &id);
                    Err(BuckyError::from(BuckyErrorCode::NotMatch))
                }
            }
            Err(e) => {
                error!("get {} from meta failed, err {}", &id, e);
                Err(e)
            }
        }
    }
}

enum SaveTarget {
    Meta(MetaClient, Option<(StandardObject, PrivateKey)>),
    File(String, Option<(StandardObject, PrivateKey)>),
}

impl SaveTarget {
    fn owner_id(&self) -> Option<ObjectId> {
        match self {
            SaveTarget::Meta(_, owner) => owner.as_ref().map(|(obj, _)| obj.calculate_id()),
            SaveTarget::File(_, owner) => owner.as_ref().map(|(obj, _)| obj.calculate_id()),
        }
    }
}

async fn save_obj<D, N>(target: SaveTarget, obj: &N)
where
    D: ObjectType,
    N: RawEncode,
    N: NamedObject<D>,
    <D as ObjectType>::ContentType: BodyContent,
{
    match target {
        SaveTarget::Meta(meta_client, owner) => {
            let upload_data = SavedMetaObject::Data(Data {
                id: obj.desc().calculate_id(),
                data: obj.to_vec().unwrap(),
            });

            match meta_client
                .update_desc(
                    &owner.as_ref().unwrap().0,
                    &upload_data,
                    None,
                    None,
                    &owner.as_ref().unwrap().1,
                )
                .await
            {
                Ok(txid) => {
                    info!("upload obj to meta success, TxId {}", &txid);
                }
                Err(e) => {
                    error!("upload obj to meta fail, err {}", e);
                }
            }
        }
        SaveTarget::File(path, _) => match obj.encode_to_file(path.as_ref(), false) {
            Ok(_) => {
                info!("write obj to {} success", path);
            }
            Err(e) => {
                info!("write obj to {} failed, err {}", path, e);
            }
        },
    }
}

lazy_static! {
    static ref DEFAULT_TARGET: String = MetaMinerTarget::default().to_string();
}

#[async_std::main]
async fn main() -> BuckyResult<()> {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Debug)
        .init()
        .unwrap();
    // let default_target = MetaMinerTarget::default().to_string();
    let meta_arg = Arg::with_name("meta_target")
        .short("m")
        .long("meta_target")
        .default_value(&DEFAULT_TARGET)
        .help("meta target");
    let matches = App::new("app-tool")
        .version(cyfs_base::get_version())
        .about("manage app list")
        .subcommand(
            SubCommand::with_name("list")
                .about("create/modify/show app list object")
                .subcommand(
                    SubCommand::with_name("create")
                        .about("create app list object")
                        .arg(
                            Arg::with_name("owner")
                                .short("o")
                                .long("owner")
                                .takes_value(true)
                                .required(true)
                                .help("app list owner desc/sec file"),
                        )
                        .arg(
                            Arg::with_name("name")
                                .short("n")
                                .long("name")
                                .default_value("")
                                .help("app list name, default empty string"),
                        )
                        .arg(
                            Arg::with_name("type")
                                .short("t")
                                .long("type")
                                .default_value("app")
                                .possible_values(&["app", "service"]),
                        )
                        .arg(
                            Arg::with_name("upload")
                                .short("u")
                                .long("upload")
                                .help("upload app list to chain, use owner account"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    add_id_or_file_arg(SubCommand::with_name("put").about("put app to app list"))
                        .arg(
                            Arg::with_name("type")
                                .short("t")
                                .long("type")
                                .default_value("app")
                                .possible_values(&["app", "service"]),
                        )
                        .arg(
                            Arg::with_name("appid")
                                .short("i")
                                .required(true)
                                .takes_value(true)
                                .help("app id add to app list"),
                        )
                        .arg(
                            Arg::with_name("appver")
                                .short("v")
                                .required(true)
                                .takes_value(true)
                                .help("app ver add to app list"),
                        )
                        .arg(
                            Arg::with_name("status")
                                .short("s")
                                .long("start")
                                .help("start app, default false"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    add_id_or_file_arg(
                        SubCommand::with_name("remove").about("remove app from list"),
                    )
                    .arg(
                        Arg::with_name("type")
                            .short("t")
                            .long("type")
                            .default_value("app")
                            .possible_values(&["app", "service"]),
                    )
                    .arg(
                        Arg::with_name("appid")
                            .short("i")
                            .takes_value(true)
                            .required(true)
                            .help("remove app id"),
                    )
                    .arg(meta_arg.clone()),
                )
                .subcommand(
                    add_id_or_file_arg(SubCommand::with_name("clear").about("clean all apps"))
                        .arg(
                            Arg::with_name("type")
                                .short("t")
                                .long("type")
                                .default_value("app")
                                .possible_values(&["app", "service"]),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    add_id_or_file_arg(SubCommand::with_name("show").about("show app list obj"))
                        .arg(
                            Arg::with_name("type")
                                .short("t")
                                .long("type")
                                .default_value("app")
                                .possible_values(&["app", "service"]),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    add_id_or_file_arg(
                        SubCommand::with_name("update").about("update app list from json"),
                    )
                    .arg(
                        Arg::with_name("config")
                            .short("c")
                            .long("config")
                            .takes_value(true)
                            .required(true),
                    )
                    .arg(
                        Arg::with_name("type")
                            .short("t")
                            .long("type")
                            .default_value("app")
                            .possible_values(&["app", "service"]),
                    )
                    .arg(meta_arg.clone()),
                ),
        )
        .subcommand(
            SubCommand::with_name("app")
                .about("create/modify/show app object")
                .subcommand(
                    SubCommand::with_name("create")
                        .about("create app object")
                        .arg(
                            Arg::with_name("owner")
                                .short("o")
                                .long("owner")
                                .takes_value(true)
                                .required(true)
                                .help("app owner desc/sec file"),
                        )
                        .arg(
                            Arg::with_name("id")
                                .index(1)
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(
                            Arg::with_name("upload")
                                .short("u")
                                .long("upload")
                                .help("upload app list to chain, use owner account"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    add_id_or_file_arg(SubCommand::with_name("set").about("add source to app"))
                        .arg(
                            Arg::with_name("appver")
                                .short("v")
                                .required(true)
                                .takes_value(true)
                                .help("ver add to app source"),
                        )
                        .arg(
                            Arg::with_name("source")
                                .short("s")
                                .required(true)
                                .takes_value(true)
                                .help("fileid add to app source"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    add_id_or_file_arg(
                        SubCommand::with_name("remove").about("remove source from app"),
                    )
                    .arg(
                        Arg::with_name("appver")
                            .short("v")
                            .takes_value(true)
                            .required(true)
                            .help("remove app ver"),
                    )
                    .arg(meta_arg.clone()),
                )
                .subcommand(
                    add_id_or_file_arg(SubCommand::with_name("clear").about("clean all sources"))
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    add_id_or_file_arg(SubCommand::with_name("show").about("show app obj"))
                        .arg(meta_arg.clone()),
                ),
        )
        .subcommand(
            SubCommand::with_name("cmd")
                .about("add/install/uninstall/start/stop/setpermission/setquota")
                .subcommand(
                    SubCommand::with_name("add")
                        .about("add app")
                        .arg(
                            Arg::with_name("owner")
                                .short("o")
                                .long("owner")
                                .takes_value(true)
                                .required(true)
                                .help("app owner id"),
                        )
                        .arg(
                            Arg::with_name("id")
                                .index(1)
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    SubCommand::with_name("install")
                        .about("install app")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(
                            Arg::with_name("version")
                                .short("value")
                                .long("version")
                                .takes_value(true)
                                .required(true)
                                .help("app version"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    SubCommand::with_name("start")
                        .about("install app")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    SubCommand::with_name("stop")
                        .about("stop app")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    SubCommand::with_name("remove")
                        .about("remove app")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    SubCommand::with_name("uninstall")
                        .about("uninstall app")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    SubCommand::with_name("setpermission")
                        .about("set app permission")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(meta_arg.clone()),
                )
                .subcommand(
                    SubCommand::with_name("setquota")
                        .about("set app quota")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(meta_arg.clone()),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        ("list", Some(matches)) => {
            match matches.subcommand() {
                ("create", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let meta_client = MetaClient::new_target(
                            MetaMinerTarget::from_str(matches.value_of("meta_target").unwrap())
                                .unwrap(),
                        );
                        let owner_path = Path::new(matches.value_of("owner").unwrap());
                        let (desc, sec) = cyfs_util::get_desc_from_file(
                            &owner_path.with_extension("desc"),
                            &owner_path.with_extension("sec"),
                        )
                        .unwrap();
                        let owner_id = desc.calculate_id();
                        let list = AppList::create(
                            owner_id,
                            matches.value_of("name").unwrap(),
                            matches.value_of("type").unwrap(),
                        );
                        let id = list.desc().calculate_id();
                        if matches.is_present("upload") {
                            let upload_data = SavedMetaObject::Data(Data {
                                id: id.clone(),
                                data: list.to_vec().unwrap(),
                            });
                            match meta_client
                                .create_desc(&desc, &upload_data, 0, 0, 0, &sec)
                                .await
                            {
                                Ok(txid) => {
                                    info!(
                                        "upload app list {} to meta success, TxId {}",
                                        &id, &txid
                                    );
                                }
                                Err(e) => {
                                    error!("upload app list {} to meta fail, err {}", &id, e);
                                }
                            }
                        } else {
                            let path = Path::new(&id.to_string()).with_extension("obj");
                            match list.encode_to_file(&path, false) {
                                Ok(_) => {
                                    info!("write app list to file {} success", path.display());
                                }
                                Err(e) => {
                                    error!(
                                        "write app list to file {} fail, err {}",
                                        path.display(),
                                        e
                                    );
                                }
                            }
                        }
                    })
                    .await;
                }
                ("put", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (mut list, target) = get_list(&matches).await.unwrap();

                        let app_id =
                            DecAppId::from_str(matches.value_of("appid").unwrap()).unwrap();
                        let ver = matches.value_of("appver").unwrap().to_owned();
                        let status = AppStatus::create(
                            target.owner_id().unwrap(),
                            app_id,
                            ver,
                            matches.is_present("status"),
                        );
                        list.put(status);
                        save_obj(target, &list).await;
                    })
                    .await;
                }
                ("remove", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (mut list, target) = get_list(&matches).await.unwrap();

                        let app_id =
                            DecAppId::from_str(matches.value_of("appid").unwrap()).unwrap();
                        list.remove(&app_id);

                        save_obj(target, &list).await;
                    })
                    .await;
                }
                ("clear", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (mut list, target) = get_list(&matches).await.unwrap();

                        list.clear();

                        save_obj(target, &list).await;
                    })
                    .await;
                }
                ("show", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (list, _) = get_list(&matches).await.unwrap();

                        println!("desc type: App List");
                        println!("owner: {}", list.desc().owner().unwrap());
                        println!("id: {}", list.id());
                        println!("category: {}", list.category());
                        for (id, status) in list.app_list() {
                            println!(
                                "app {}: ver {}, start {}",
                                &id,
                                status.version(),
                                status.status()
                            )
                        }
                    })
                    .await;
                }
                ("update", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (mut list, target) = get_list(&matches).await.unwrap();
                        let config: serde_json::Value = serde_json::from_reader(
                            std::fs::File::open(matches.value_of("config").unwrap()).unwrap(),
                        )
                        .unwrap();
                        // 需要config格式：[{id, ver, status}]
                        for service in config.as_array().unwrap() {
                            let app_id = DecAppId::from_str(
                                service
                                    .as_object()
                                    .unwrap()
                                    .get("id")
                                    .unwrap()
                                    .as_str()
                                    .unwrap(),
                            )
                            .unwrap();
                            let ver = service
                                .as_object()
                                .unwrap()
                                .get("ver")
                                .unwrap()
                                .as_str()
                                .unwrap()
                                .to_owned();
                            let status = service
                                .as_object()
                                .unwrap()
                                .get("status")
                                .unwrap()
                                .as_i64()
                                .unwrap()
                                == 1;
                            let status =
                                AppStatus::create(target.owner_id().unwrap(), app_id, ver, status);
                            list.put(status);
                        }

                        save_obj(target, &list).await;
                    })
                    .await;
                }
                v @ _ => {
                    error!("unknown list command: {}", v.0);
                    std::process::exit(1);
                }
            }
        }
        ("app", Some(matches)) => match matches.subcommand() {
            ("create", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let meta_client = MetaClient::new_target(
                        MetaMinerTarget::from_str(matches.value_of("meta_target").unwrap())
                            .unwrap(),
                    );
                    let owner_path = Path::new(matches.value_of("owner").unwrap());
                    let (desc, sec) = cyfs_util::get_desc_from_file(
                        &owner_path.with_extension("desc"),
                        &owner_path.with_extension("sec"),
                    )
                    .unwrap();
                    let owner_id = desc.calculate_id();
                    let app_id = matches.value_of("id").unwrap();
                    let app = DecApp::create(owner_id, app_id);
                    let id = app.desc().calculate_id();
                    if matches.is_present("upload") {
                        let upload_data = SavedMetaObject::Data(Data {
                            id: id.clone(),
                            data: app.to_vec().unwrap(),
                        });
                        match meta_client
                            .create_desc(&desc, &upload_data, 0, 0, 0, &sec)
                            .await
                        {
                            Ok(txid) => {
                                info!("upload app {} to meta success, TxId {}", &id, &txid);
                            }
                            Err(e) => {
                                error!("upload app {} to meta fail, err {}", &id, e);
                            }
                        }
                    } else {
                        let path = Path::new(&id.to_string()).with_extension("obj");
                        match app.encode_to_file(&path, false) {
                            Ok(_) => {
                                info!("write app to file {} success", path.display());
                            }
                            Err(e) => {
                                error!("write app to file {} fail, err {}", path.display(), e);
                            }
                        }
                    }
                })
                .await;
            }
            ("set", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let (mut app, target) = get_app(&matches).await.unwrap();

                    let source = ObjectId::from_str(matches.value_of("source").unwrap()).unwrap();
                    let ver = matches.value_of("appver").unwrap().to_owned();
                    app.set_source(ver, source, None);

                    save_obj(target, &app).await;
                })
                .await;
            }
            ("remove", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let (mut app, target) = get_app(&matches).await.unwrap();

                    let ver = matches.value_of("appver").unwrap();
                    app.remove_source(&ver);

                    save_obj(target, &app).await;
                })
                .await;
            }
            ("clear", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let (mut app, target) = get_app(&matches).await.unwrap();

                    app.clear_source();

                    save_obj(target, &app).await;
                })
                .await;
            }
            ("show", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let (app, _) = get_app(&matches).await.unwrap();

                    println!("desc type: Dec App");
                    println!("name {}", app.name());
                    for (ver, source) in app.source() {
                        println!("app have source: {} : {}", ver, source);
                    }
                })
                .await;
            }
            v @ _ => {
                error!("unknown list command: {}", v.0);
                std::process::exit(1);
            }
        },
        v @ _ => {
            error!("unknown command: {}", v.0);
            std::process::exit(1);
        }
    }

    Ok(())
}

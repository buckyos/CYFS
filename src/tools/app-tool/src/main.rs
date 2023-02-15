use clap::{App, Arg, ArgMatches, SubCommand};
use cyfs_base::{
    BodyContent, BuckyError, BuckyErrorCode, BuckyResult, FileDecoder, FileEncoder, NamedObject,
    ObjectDesc, ObjectId, ObjectType, OwnerObjectDesc, PrivateKey, RawConvertTo, RawEncode,
    RawFrom, StandardObject,
};
use cyfs_base_meta::{Data, SavedMetaObject};
use cyfs_core::{AppList, AppListObj, AppStatus, AppStatusObj, DecApp, DecAppId, DecAppObj};
use cyfs_meta_lib::{MetaClient, MetaMinerTarget};
use lazy_static::lazy_static;
use log::*;
use std::path::Path;
use std::str::FromStr;

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

async fn get_app_from_meta(id: &ObjectId, target: &SaveTarget) -> BuckyResult<DecApp> {
    match target {
        SaveTarget::Meta(client, _) => {
            match client.get_desc(&id).await {
                Ok(data) => {
                    if let SavedMetaObject::Data(data) = data {
                        let app = DecApp::clone_from_slice(&data.data).unwrap();
                        Ok(app)
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
        },
        SaveTarget::File(_, _) => {
            Err(BuckyError::from(BuckyErrorCode::NotSupport))
        }
    }
}

async fn get_app(matches: &ArgMatches<'_>) -> BuckyResult<(DecApp, SaveTarget)> {
    let owner = get_owner(matches);
    if let Some(id_str) = matches.value_of("id") {
        let id = ObjectId::from_str(id_str).unwrap();
        let meta_client = MetaClient::new_target(
            MetaMinerTarget::from_str(matches.value_of("meta_target").unwrap()).unwrap(),
        );

        let target = SaveTarget::Meta(meta_client, owner);
        let app = get_app_from_meta(&id, &target).await?;
        Ok((app, target))
    } else if let Some(file_path) = matches.value_of("file") {
        let (app, _) = DecApp::decode_from_file(file_path.as_ref(), &mut vec![])?;
        return Ok((app, SaveTarget::File(file_path.to_owned(), owner)));
    } else {
        let meta_client = MetaClient::new_target(
            MetaMinerTarget::from_str(matches.value_of("meta_target").unwrap()).unwrap(),
        );

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

    async fn save_obj<D, N>(&self, obj: &N) -> BuckyResult<()>
        where
            D: ObjectType,
            N: RawEncode,
            N: NamedObject<D>,
            <D as ObjectType>::ContentType: BodyContent,
    {
        match self {
            SaveTarget::Meta(meta_client, owner) => {
                let upload_data = SavedMetaObject::Data(Data {
                    id: obj.desc().calculate_id(),
                    data: obj.to_vec().unwrap(),
                });

                if let Some((owner, secret)) = owner {
                    match meta_client.update_desc(owner,&upload_data,None,None, secret).await
                    {
                        Ok(txid) => {
                            info!("upload obj to meta success, TxId {}", &txid);
                        }
                        Err(e) => {
                            error!("upload obj to meta fail, err {}", e);
                            return Err(e);
                        }
                    }
                } else {
                    error!("save obj to meta but no owner desc!");
                    return Err(BuckyError::from(BuckyErrorCode::InvalidParam))
                }
            }
            SaveTarget::File(path, _) => match obj.encode_to_file(path.as_ref(), false) {
                Ok(_) => {
                    info!("write obj to {} success", path);
                }
                Err(e) => {
                    info!("write obj to {} failed, err {}", path, e);
                    return Err(e)
                }
            },
        }

        Ok(())
    }
}



lazy_static! {
    static ref DEFAULT_TARGET: String = MetaMinerTarget::default().to_string();
}


async fn main_run() -> BuckyResult<()> {
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
    let app = App::new("app-tool")
        .version(cyfs_base::get_version())
        .about("manage app list")
        .subcommand(
            SubCommand::with_name("list")
                .about("create/modify/show app list object")
                .subcommand(SubCommand::with_name("create")
                        .about("create app list object")
                        .arg(Arg::with_name("owner")
                                .short("o").long("owner")
                                .takes_value(true).required(true)
                                .help("app list owner desc/sec file"))
                        .arg(Arg::with_name("name")
                                .short("n").long("name")
                                .default_value("")
                                .help("app list name, default empty string"))
                        .arg(Arg::with_name("type")
                                .short("t").long("type")
                                .default_value("app")
                                .possible_values(&["app", "service"]))
                        .arg(Arg::with_name("upload")
                                .short("u").long("upload")
                                .help("upload app list to chain, use owner account"))
                        .arg(meta_arg.clone()),
                )
                .subcommand(add_id_or_file_arg(SubCommand::with_name("put").about("put app to app list"))
                        .arg(Arg::with_name("type")
                                .short("t").long("type")
                                .default_value("app")
                                .possible_values(&["app", "service"]))
                        .arg(Arg::with_name("appid")
                                .short("i")
                                .required(true).takes_value(true)
                                .help("app id add to app list"))
                        .arg(Arg::with_name("appver")
                                .short("v")
                                .required(true).takes_value(true)
                                .help("app ver add to app list"))
                        .arg(Arg::with_name("status")
                                .short("s").long("start")
                                .help("start app, default false"))
                        .arg(meta_arg.clone()),
                )
                .subcommand(add_id_or_file_arg(SubCommand::with_name("remove").about("remove app from list"))
                    .arg(Arg::with_name("type")
                            .short("t").long("type")
                            .default_value("app")
                            .possible_values(&["app", "service"]))
                    .arg(Arg::with_name("appid")
                            .short("i")
                            .takes_value(true).required(true)
                            .help("remove app id"))
                    .arg(meta_arg.clone()),
                )
                .subcommand(add_id_or_file_arg(SubCommand::with_name("clear").about("clean all apps"))
                        .arg(Arg::with_name("type")
                                .short("t").long("type")
                                .default_value("app")
                                .possible_values(&["app", "service"]))
                        .arg(meta_arg.clone()),
                )
                .subcommand(add_id_or_file_arg(SubCommand::with_name("show").about("show app list obj"))
                        .arg(Arg::with_name("type")
                                .short("t").long("type")
                                .default_value("app")
                                .possible_values(&["app", "service"]))
                        .arg(meta_arg.clone()),
                )
                .subcommand(add_id_or_file_arg(SubCommand::with_name("update").about("update app list from json"))
                    .arg(Arg::with_name("config")
                            .short("c").long("config")
                            .takes_value(true)
                            .required(true))
                    .arg(Arg::with_name("type").short("t").long("type").default_value("app").possible_values(&["app", "service"]))
                    .arg(Arg::with_name("clear").long("clear")
                        .help("cleat list before update"))
                    .arg(Arg::with_name("unpreview").long("unpreview")
                        .help("change app preview version to normal version"))
                    .arg(meta_arg.clone()),
                ),
        )
        .subcommand(SubCommand::with_name("app")
                .about("create/modify/show app object")
                .subcommand(SubCommand::with_name("create")
                        .about("create app object")
                        .arg(Arg::with_name("owner")
                                .short("o")
                                .long("owner")
                                .takes_value(true)
                                .required(true)
                                .help("app owner desc/sec file"))
                        .arg(Arg::with_name("id")
                                .index(1)
                                .takes_value(true)
                                .required(true)
                                .help("app id"))
                        .arg(Arg::with_name("upload")
                                .short("u")
                                .long("upload")
                                .help("upload app list to chain, use owner account"))
                        .arg(meta_arg.clone()),
                )
                .subcommand(add_id_or_file_arg(SubCommand::with_name("set").about("add source to app"))
                        .arg(Arg::with_name("appver")
                                .short("v")
                                .required(true)
                                .takes_value(true)
                                .help("ver add to app source"))
                        .arg(Arg::with_name("source")
                                .short("s")
                                .required(true)
                                .takes_value(true)
                                .help("fileid add to app source"))
                        .arg(meta_arg.clone()),
                )
                .subcommand(add_id_or_file_arg(SubCommand::with_name("remove").about("remove source from app"))
                    .arg(Arg::with_name("appver")
                            .short("v")
                            .takes_value(true)
                            .required(true)
                            .help("remove app ver"))
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
        );
        let matches = app.get_matches();

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
                                    return Err(e);
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
                                    return Err(e);
                                }
                            }
                        }
                        Ok(())
                    })
                    .await
                }
                ("put", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (mut list, target) = get_list(&matches).await?;

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
                        target.save_obj(&list).await
                    })
                    .await
                }
                ("remove", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (mut list, target) = get_list(&matches).await?;

                        let app_id =
                            DecAppId::from_str(matches.value_of("appid").unwrap()).unwrap();
                        list.remove(&app_id);

                        target.save_obj(&list).await
                    })
                    .await
                }
                ("clear", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (mut list, target) = get_list(&matches).await?;

                        list.clear();

                        target.save_obj(&list).await
                    })
                    .await
                }
                ("show", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (list, _) = get_list(&matches).await?;

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
                        Ok(())
                    })
                    .await
                }
                ("update", Some(matches)) => {
                    let matches = matches.clone();
                    async_std::task::spawn(async move {
                        let (mut list, target) = get_list(&matches).await?;
                        let config: serde_json::Value = serde_json::from_reader(
                            std::fs::File::open(matches.value_of("config").unwrap()).unwrap(),
                        )
                        .unwrap();
                        if matches.is_present("clear") {
                            list.clear();
                        }
                        // 需要config格式：[{id, ver, status}]
                        for service in config.as_array().unwrap() {
                            let service = service.as_object().unwrap();
                            let app_id = DecAppId::from_str(
                                service.get("id").unwrap().as_str().unwrap(),
                            )
                            .unwrap();
                            let ver = service.get("ver").unwrap().as_str().unwrap();
                            let status = service.get("status").unwrap().as_i64().unwrap() == 1;

                            if matches.is_present("unpreview") {
                                info!("check preview version {} for app {}", ver, &app_id);

                                if let Ok(mut app) = get_app_from_meta(app_id.object_id(), &target).await {
                                    let pre_version = format!("{}-preview", ver);
                                    if let Ok(id) = app.find_source(&pre_version) {
                                        info!("find preview version {}, change to normal", &pre_version);
                                        let desc = app.find_source_desc(&pre_version).map(|s|s.to_owned());
                                        app.remove_source(&pre_version);
                                        app.set_source(ver.to_owned(), id, desc);

                                        target.save_obj(&app).await;
                                    }
                                }
                            }

                            let status =
                                AppStatus::create(target.owner_id().unwrap(), app_id, ver.to_owned(), status);
                            list.put(status);
                        }

                        target.save_obj(&list).await
                    })
                    .await
                }
                v @ _ => {
                    error!("unknown list command: {}", v.0);
                    Err(BuckyError::new(BuckyErrorCode::NotSupport, v.0))
                    // std::process::exit(1);
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
                                return Err(e)
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
                                return Err(e)
                            }
                        }
                    }
                    Ok(())
                }).await
            }
            ("set", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let (mut app, target) = get_app(&matches).await?;

                    let source = ObjectId::from_str(matches.value_of("source").unwrap()).unwrap();
                    let ver = matches.value_of("appver").unwrap().to_owned();
                    app.set_source(ver, source, None);

                    target.save_obj(&app).await
                })
                .await
            }
            ("remove", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let (mut app, target) = get_app(&matches).await?;

                    let ver = matches.value_of("appver").unwrap();
                    app.remove_source(&ver);

                    target.save_obj(&app).await
                })
                .await
            }
            ("clear", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let (mut app, target) = get_app(&matches).await?;

                    app.clear_source();

                    target.save_obj(&app).await
                })
                .await
            }
            ("show", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let (app, _) = get_app(&matches).await?;

                    println!("desc type: Dec App");
                    println!("name {}", app.name());
                    for (ver, source) in app.source() {
                        println!("app have source: {} : {}", ver, source);
                    }
                    Ok(())
                })
                .await
            }
            v @ _ => {
                error!("unknown list command: {}", v.0);
                Err(BuckyError::new(BuckyErrorCode::NotSupport, v.0))
                //std::process::exit(1);
            }
        },
        v @ _ => {
            error!("unknown command: {}", v.0);
            Err(BuckyError::new(BuckyErrorCode::NotSupport, v.0))
            //std::process::exit(1);
        }
    }
}

fn main() -> BuckyResult<()> {
    cyfs_debug::ProcessDeadHelper::patch_task_min_thread();

    async_std::task::block_on(main_run())
}

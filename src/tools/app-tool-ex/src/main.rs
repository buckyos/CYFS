use chrono::{DateTime, Local};
use clap::{App, Arg, ArgMatches, SubCommand};
use cyfs_base::*;
use cyfs_core::*;
use log::*;
use std::path::Path;
use std::str::FromStr;

mod non_helper;
use crate::non_helper::NonHelper;

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

#[async_std::main]
async fn main() -> BuckyResult<()> {
    simple_logger::SimpleLogger::new()
        .with_level(LevelFilter::Debug)
        .init()
        .unwrap();

    let now_time = bucky_time_to_system_time(bucky_time_now());
    let now_time: DateTime<Local> = now_time.into();
    info!("now time:{}", now_time.format("[%Y-%m-%d %H:%M:%S.%3f]"));

    let matches = App::new("app-tool-ex")
        .version(cyfs_base::get_version())
        .about("app-tool-ex")
        .subcommand(SubCommand::with_name("list").about("show app list object"))
        .subcommand(SubCommand::with_name("cmd_list").about("show app cmd list object"))
        .subcommand(
            SubCommand::with_name("cmd")
                .about("add/install/uninstall/start/stop/setpermission/setquota/setautoupdate")
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
                        ),
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
                                .short("ver")
                                .long("version")
                                .takes_value(true)
                                .required(true)
                                .help("app version"),
                        )
                        .arg(
                            Arg::with_name("run")
                                .takes_value(true)
                                .required(false)
                                .default_value("1")
                                .help("run after install, [0 or 1]"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("start").about("start app").arg(
                        Arg::with_name("id")
                            .takes_value(true)
                            .required(true)
                            .help("app id"),
                    ),
                )
                .subcommand(
                    SubCommand::with_name("stop").about("stop app").arg(
                        Arg::with_name("id")
                            .takes_value(true)
                            .required(true)
                            .help("app id"),
                    ),
                )
                .subcommand(
                    SubCommand::with_name("remove").about("remove app").arg(
                        Arg::with_name("id")
                            .takes_value(true)
                            .required(true)
                            .help("app id"),
                    ),
                )
                .subcommand(
                    SubCommand::with_name("uninstall")
                        .about("uninstall app")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("setpermission")
                        .about("set app permission")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("setquota")
                        .about("set app quota")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        ),
                )
                .subcommand(
                    SubCommand::with_name("setautoupdate")
                        .about("set app autoupdate")
                        .arg(
                            Arg::with_name("id")
                                .takes_value(true)
                                .required(true)
                                .help("app id"),
                        )
                        .arg(
                            Arg::with_name("autoupdate")
                                .takes_value(true)
                                .required(true)
                                .help("set auto update, [0 or 1]"),
                        ),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        ("list", Some(_matches)) => {
            async_std::task::spawn(async move {
                let mut helper = NonHelper::new();
                let _ = helper.init().await;
                if let Ok(app_list) = helper.get_app_local_list().await {
                    let list = app_list.app_list();
                    let mut output = format!("===>");
                    for app_id in list {
                        if let Ok(status) = helper.get_local_status(app_id).await {
                            //info!("===> appId:{}, status: {}, version:{:?}", app_id, status.status(), status.version());
                            output = format!("{}\n{}", output, status.output());
                        } else {
                            warn!("===> appId:{}, status not found!", app_id);
                            output = format!("{}\nappId:{}, status not found!", output, app_id);
                        }
                    }
                    info!("{}", output);
                } else {
                    warn!("===> app list not found!");
                }
            })
            .await;
        }
        ("cmd_list", Some(_matches)) => {
            async_std::task::spawn(async move {
                let mut helper = NonHelper::new();
                let _ = helper.init().await;
                if let Ok(cmd_list) = helper.get_app_cmd_list().await {
                    info!("app cmd list: {}", cmd_list.output());
                } else {
                    warn!("===> app cmd list not found!");
                }
            })
            .await;
        }
        ("cmd", Some(matches)) => match matches.subcommand() {
            ("add", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let mut helper = NonHelper::new();
                    let _ = helper.init().await;
                    let id = DecAppId::from_str(matches.value_of("id").unwrap()).unwrap();
                    let owner_id = ObjectId::from_str(matches.value_of("owner").unwrap()).unwrap();
                    let cmd = AppCmd::add(helper.get_owner(), id, Some(owner_id));
                    match helper
                        .post_object_without_resp(
                            &cmd,
                            Some(CYFS_SYSTEM_APP_CMD_VIRTUAL_PATH.to_owned()),
                        )
                        .await
                    {
                        Ok(_) => {
                            info!("===> add success");
                        }
                        Err(e) => {
                            warn!("===> add app failed, err:{}", e);
                        }
                    };
                })
                .await;
            }
            ("install", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let mut helper = NonHelper::new();
                    let _ = helper.init().await;
                    let id = DecAppId::from_str(matches.value_of("id").unwrap()).unwrap();
                    let ver = matches.value_of("version").unwrap();
                    let mut run = false;
                    if matches.value_of("run").unwrap() == "1" {
                        run = true;
                    }
                    let cmd = AppCmd::install(helper.get_owner(), id, ver, run);
                    match helper
                        .post_object_without_resp(
                            &cmd,
                            Some(CYFS_SYSTEM_APP_CMD_VIRTUAL_PATH.to_owned()),
                        )
                        .await
                    {
                        Ok(_) => {
                            info!("===> install success");
                        }
                        Err(e) => {
                            warn!("===> install app failed, err:{}", e);
                        }
                    };
                })
                .await;
            }
            ("uninstall", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let mut helper = NonHelper::new();
                    let _ = helper.init().await;
                    let id = DecAppId::from_str(matches.value_of("id").unwrap()).unwrap();
                    let cmd = AppCmd::uninstall(helper.get_owner(), id);
                    match helper
                        .post_object_without_resp(
                            &cmd,
                            Some(CYFS_SYSTEM_APP_CMD_VIRTUAL_PATH.to_owned()),
                        )
                        .await
                    {
                        Ok(_) => {
                            info!("===> uninstall success");
                        }
                        Err(e) => {
                            warn!("===> uninstall app failed, err:{}", e);
                        }
                    };
                })
                .await;
            }
            ("start", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let mut helper = NonHelper::new();
                    let _ = helper.init().await;
                    let id = DecAppId::from_str(matches.value_of("id").unwrap()).unwrap();
                    let cmd = AppCmd::start(helper.get_owner(), id);
                    match helper
                        .post_object_without_resp(
                            &cmd,
                            Some(CYFS_SYSTEM_APP_CMD_VIRTUAL_PATH.to_owned()),
                        )
                        .await
                    {
                        Ok(_) => {
                            info!("===> start success");
                        }
                        Err(e) => {
                            warn!("===> start app failed, err:{}", e);
                        }
                    };
                })
                .await;
            }
            ("stop", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let mut helper = NonHelper::new();
                    let _ = helper.init().await;
                    let id = DecAppId::from_str(matches.value_of("id").unwrap()).unwrap();
                    let cmd = AppCmd::stop(helper.get_owner(), id);
                    match helper
                        .post_object_without_resp(
                            &cmd,
                            Some(CYFS_SYSTEM_APP_CMD_VIRTUAL_PATH.to_owned()),
                        )
                        .await
                    {
                        Ok(_) => {
                            info!("===> stop success");
                        }
                        Err(e) => {
                            warn!("===> stop app failed, err:{}", e);
                        }
                    };
                })
                .await;
            }
            ("remove", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let mut helper = NonHelper::new();
                    let _ = helper.init().await;
                    let id = DecAppId::from_str(matches.value_of("id").unwrap()).unwrap();
                    let cmd = AppCmd::remove(helper.get_owner(), id);
                    match helper
                        .post_object_without_resp(
                            &cmd,
                            Some(CYFS_SYSTEM_APP_CMD_VIRTUAL_PATH.to_owned()),
                        )
                        .await
                    {
                        Ok(_) => {
                            info!("===> remove success");
                        }
                        Err(e) => {
                            warn!("===> remove app failed, err:{}", e);
                        }
                    };
                })
                .await;
            }
            ("setautoupdate", Some(matches)) => {
                let matches = matches.clone();
                async_std::task::spawn(async move {
                    let mut helper = NonHelper::new();
                    let _ = helper.init().await;
                    let id = DecAppId::from_str(matches.value_of("id").unwrap()).unwrap();
                    let mut autoupdate = true;
                    if matches.value_of("autoupdate").unwrap() == "0" {
                        autoupdate = false;
                    }
                    let cmd = AppCmd::set_auto_update(helper.get_owner(), id, autoupdate);
                    match helper
                        .post_object_without_resp(
                            &cmd,
                            Some(CYFS_SYSTEM_APP_CMD_VIRTUAL_PATH.to_owned()),
                        )
                        .await
                    {
                        Ok(_) => {
                            info!("===> set autoupdate success");
                        }
                        Err(e) => {
                            warn!("===> set autoupdate failed, err:{}", e);
                        }
                    };
                })
                .await;
            }
            v @ _ => {
                error!("unknown cmd command: {}", v.0);
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

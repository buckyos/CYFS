use clap::{App, SubCommand, Arg, ArgMatches};
use crate::util::{get_objids_from_matches, get_eps_from_matches, get_deviceids_from_matches};
use log::*;
use cyfs_base::{StandardObject, FileDecoder, FileEncoder, NamedObject, AnyNamedObject, ObjectDesc, ObjectId, OwnerObjectDesc};
use cyfs_core::{CoreObjectType, DecApp, DecAppObj, AppList, AppStatus, AppListObj, AppStatusObj, DecAppId};
use std::convert::TryFrom;
use std::str::FromStr;

pub fn modify_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("modify").about("modify desc")
        .arg(Arg::with_name("desc").required(true).index(1).help("desc file to modify"))
        .arg(Arg::with_name("sn").short("s").long("sn").value_delimiter(";").help("new sn list"))
        .arg(Arg::with_name("eps").long("eps").short("e").value_delimiter(";").help("new endpoint list"))
        .arg(Arg::with_name("members").long("members").short("m").value_delimiter(";").help("members set to simple group"))
        .arg(Arg::with_name("add_members").long("add").short("a").value_delimiter(";").help("members append to simple group"))
        .arg(Arg::with_name("add_oods").long("add_ood").short("o").value_delimiter(";").help("device id append to people"))
        .arg(Arg::with_name("ood_lists").long("ood_lists").short("l").value_delimiter(";").help("device id set to people"))
        .arg(Arg::with_name("name").short("n").long("name").takes_value(true).help("people name"))
        .arg(Arg::with_name("source").long("source").value_delimiter(";").help("add source to app, {ver}:{id}"))
        .arg(Arg::with_name("app_id").long("appid").takes_value(true).help("app id add to app list"))
        .arg(Arg::with_name("app_ver").long("appver").takes_value(true).help("app ver add to app list"))
        .arg(Arg::with_name("app_status").long("appstart").help("start app, default false"))
}

pub fn modify_desc(matches: &ArgMatches) {
    let path = matches.value_of("desc").unwrap();
    match AnyNamedObject::decode_from_file(path.as_ref(), &mut vec![]) {
        Ok((desc, _)) => {
            match desc {
                AnyNamedObject::Standard(mut obj) => {
                    match obj {
                        StandardObject::Device(ref mut p) => {
                            if let Some(sn_list) = get_deviceids_from_matches(matches, "sn") {
                                p.body_mut().as_mut().unwrap().content_mut().mut_sn_list().clone_from(&sn_list);
                            }

                            if let Some(ep_list) = get_eps_from_matches(matches, "eps") {
                                p.body_mut().as_mut().unwrap().content_mut().mut_endpoints().clone_from(&ep_list);
                            }

                            p.encode_to_file(path.as_ref(), false).expect("write desc file err");
                            info!("modify success");
                        },
                        StandardObject::Group(mut g) => {
                            // TODO
                            // let content = g.body_mut().as_mut().unwrap().content_mut();
                            // if let Some(members) = get_objids_from_matches(matches, "members") {
                            //     content.members_mut().clone_from(&members);
                            // }

                            // if let Some(members) = get_objids_from_matches(matches, "add_members") {
                            //     for member in members {
                            //         if !content.members_mut().contains(&member) {
                            //             content.members_mut().push(member);
                            //         } else {
                            //             info!("obj {} already in group, skip.", &member);
                            //         }
                            //     }
                            // }

                            // g.encode_to_file(path.as_ref(), false).expect("write desc file err");
                        }
                        StandardObject::People(mut p) => {
                            let content = p.body_mut().as_mut().unwrap().content_mut();
                            if let Some(oods) = get_deviceids_from_matches(matches, "ood_lists") {
                                content.ood_list_mut().clone_from(&oods);
                            }

                            if let Some(oods) = get_deviceids_from_matches(matches, "add_oods") {
                                for ood in oods {
                                    if !content.ood_list_mut().contains(&ood) {
                                        content.ood_list_mut().push(ood);
                                    } else {
                                        info!("obj {} already in group, skip.", &ood);
                                    }
                                }
                            }

                            if let Some(name) = matches.value_of("name") {
                                content.set_name(name.to_owned());
                            }

                            p.encode_to_file(path.as_ref(), false).expect("write desc file err");
                        }
                        _ => {
                            error!("unsupport desc type");
                        }
                    }
                }
                AnyNamedObject::Core(obj) => {
                    match CoreObjectType::from(obj.desc().obj_type()) {
                        CoreObjectType::DecApp => {
                            let mut app = DecApp::try_from(obj).unwrap();
                            if let Some(values) = matches.values_of_lossy("source") {
                                for value in &values {
                                    let sources: Vec<&str> = value.split(":").collect();
                                    app.set_source(sources[0].to_owned(), ObjectId::from_str(sources[1]).unwrap(), None);
                                }
                            }

                            app.encode_to_file(path.as_ref(), false).expect("write desc file err");
                        },
                        CoreObjectType::AppList => {
                            let mut list = AppList::try_from(obj).unwrap();
                            let owner = list.desc().owner().unwrap();
                            if let Some(id_str) = matches.value_of("app_id") {
                                let dec_id = DecAppId::from_str(id_str).unwrap();
                                let version = matches.value_of("app_ver").unwrap().to_owned();
                                let app_status = matches.is_present("app_status");
                                let status = AppStatus::create(owner, dec_id, version, app_status);

                                list.put(status);
                            } else {
                                list.clear();
                            }

                            list.encode_to_file(path.as_ref(), false).expect("write desc file err");
                        }
                        _ => {}
                    }
                }
                AnyNamedObject::DECApp(_) => {}
            }

        },
        Err(e) => {
            error!("read desc from file {} failed, err {}", path, e);
        },
    }
}
use clap::{App, SubCommand, Arg, ArgMatches};
use crate::util::{get_eps_from_matches, get_deviceids_from_matches, get_group_members_from_matches, get_objids_from_matches};
use log::*;
use cyfs_base::{StandardObject, FileDecoder, FileEncoder, NamedObject, AnyNamedObject, ObjectDesc, ObjectId, OwnerObjectDesc, FileId, Group, BuckyError, GroupMember, BuckyErrorCode, BuckyResult, DeviceId, bucky_time_now};
use cyfs_core::{CoreObjectType, DecApp, DecAppObj, AppList, AppStatus, AppListObj, AppStatusObj, DecAppId};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::str::FromStr;

pub fn modify_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("modify").about("modify desc")
        .arg(Arg::with_name("desc").required(true).index(1).help("desc file to modify"))
        .arg(Arg::with_name("sn").short("s").long("sn").value_delimiter(";").help("new sn list"))
        .arg(Arg::with_name("eps").long("eps").short("e").value_delimiter(";").help("new endpoint list"))
        .arg(Arg::with_name("admins").long("admins").short("A").value_delimiter(";").help("set administrators to group. format [PeopleId:title]"))
        .arg(Arg::with_name("add_admins").long("add_admin").value_delimiter(";").help("append administrators to group. format [PeopleId:title]"))
        .arg(Arg::with_name("remove_admins").long("rm_admin").value_delimiter(";").help("remove administrators from group. format [PeopleId]"))
        .arg(Arg::with_name("members").long("members").short("m").value_delimiter(";").help("set members to group. format [PeopleId:title]"))
        .arg(Arg::with_name("add_members").long("add_member").value_delimiter(";").help("append members to group. format [PeopleId:title]"))
        .arg(Arg::with_name("remove_members").long("rm_member").value_delimiter(";").help("remove members from group. format [PeopleId]"))
        .arg(Arg::with_name("description").short("d").long("description").takes_value(true).help("description of group"))
        .arg(Arg::with_name("version").short("v").long("version").takes_value(true).help("version of group"))
        .arg(Arg::with_name("prev_blob_id").long("prev_blob").takes_value(true).help("prev-blob-id of group"))
        .arg(Arg::with_name("add_oods").long("add_ood").short("o").value_delimiter(";").help("device id append to people or group"))
        .arg(Arg::with_name("ood_lists").long("ood_lists").short("l").value_delimiter(";").help("device id set to people or group"))
        .arg(Arg::with_name("name").short("n").long("name").takes_value(true).help("name of people or group"))
        .arg(Arg::with_name("icon").short("I").long("icon").takes_value(true).help("icon of people or group"))
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
                            if modify_group_desc(&mut g, matches).is_ok() {
                                g.encode_to_file(path.as_ref(), false).expect("write desc file err");
                                info!("modify success");
                            }
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
                                        info!("obj {} already exist, skip.", &ood);
                                    }
                                }
                            }

                            if let Some(name) = matches.value_of("name") {
                                content.set_name(name.to_owned());
                            }

                            if let Some(icon) = matches.value_of("icon") {
                                match FileId::from_str(icon) {
                                    Ok(icon) => content.set_icon(icon),
                                    Err(_) => {
                                        warn!("invalid icon {}", icon);
                                    },
                                }
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

fn modify_group_desc(group: &mut Group, matches: &ArgMatches) -> BuckyResult<()> {
    let group_id = group.desc().object_id();

    match get_group_members_from_matches(matches, "members") {
        Ok(members) => {
            if let Some(members) = members {
                group.set_members(members);
            }
        },
        Err(err) => {
            log::error!("update group({}) failed for invalid member.", group_id);
            return Err(err);
        }
    }

    match get_group_members_from_matches(matches, "add_members") {
        Ok(additional_members) => {
            if let Some(additional_members) = additional_members {
                let mut members = HashMap::<ObjectId, String>::from_iter(group.members().iter().map(|m| (m.id, m.title.clone())));
                additional_members.into_iter().for_each(|m| {members.insert(m.id, m.title);});
                group.set_members(members.into_iter().map(|(id, title)| GroupMember::new(id, title)).collect());                
            }
        },
        Err(err) => {
            log::error!("update group({}) failed for invalid member.", group_id);
            return Err(err);
        }
    }

    if let Some(remove_members) = get_objids_from_matches(matches, "remove_members") {
        let mut members = HashMap::<ObjectId, String>::from_iter(group.members().iter().map(|m| (m.id, m.title.clone())));
        remove_members.iter().for_each(|m| {members.remove(m);});
        group.set_members(members.into_iter().map(|(id, title)| GroupMember::new(id, title)).collect());
    }
    
    match get_group_members_from_matches(matches, "admins") {
        Ok(admins) => {
            if let Some(admins) = admins {
                if group.is_simple_group() {
                    let msg = format!("update group({}) failed for the administrators of simple-group is immutable.", group_id);
                    log::error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
                }
                let org = group.check_org_body_content_mut();
                org.set_admins(admins);                
            }
        },
        Err(err) => {
            log::error!("update group({}) failed for invalid administrator.", group_id);
            return Err(err);
        }
    }

    match get_group_members_from_matches(matches, "add_admins") {
        Ok(additional_admins) => {
            if let Some(additional_admins) = additional_admins {
                if group.is_simple_group() {
                    let msg = format!("update group({}) failed for the administrators of simple-group is immutable.", group_id);
                    log::error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::Failed, msg));
                }
                let org = group.check_org_body_content_mut();
                let mut admins = HashMap::<ObjectId, String>::from_iter(org.admins().iter().map(|m| (m.id, m.title.clone())));
                additional_admins.into_iter().for_each(|m| {admins.insert(m.id, m.title);});
                org.set_admins(admins.into_iter().map(|(id, title)| GroupMember::new(id, title)).collect());                
            }
        },
        Err(err) => {
            log::error!("update group({}) failed for invalid administrator.", group_id);
            return Err(err);
        }
    }

    if let Some(remove_members) = get_objids_from_matches(matches, "remove_admins") {
        let org = group.check_org_body_content_mut();
        let mut admins = HashMap::<ObjectId, String>::from_iter(org.admins().iter().map(|m| (m.id, m.title.clone())));
        remove_members.iter().for_each(|m| {admins.remove(m);});
        org.set_admins(admins.into_iter().map(|(id, title)| GroupMember::new(id, title)).collect());
    }

    if let Some(oods) = get_deviceids_from_matches(matches, "ood_lists") {
        group.set_ood_list(oods);
    }
    if let Some(additional_oods) = get_deviceids_from_matches(matches, "add_oods") {
        let mut oods = HashSet::<DeviceId>::from_iter(group.ood_list().iter().map(|id| id.clone()));
        additional_oods.into_iter().for_each(|id| {oods.insert(id);});
        group.set_ood_list(oods.into_iter().collect());
    }

    if let Some(description) = matches.value_of("description") {
        group.set_description(Some(description.to_string()));
    }

    if let Some(icon) = matches.value_of("icon") {
        group.set_icon(Some(icon.to_string()));
    }

    if let Some(name) = matches.value_of("name") {
        group.set_name(Some(name.to_string()));
    }

    if let Some(version) = matches.value_of("version") {
        let version = match version.parse::<u64>() {
            Ok(v) => v,
            Err(e) => {
                let msg = format!("update group({}) failed for invalid version {}, err: {:?}", group_id, version, e);
                log::error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };
        group.set_version(version);
    }

    if let Some(prev_blob_id) = matches.value_of("prev_blob_id") {
        let prev_blob_id = match ObjectId::from_str(prev_blob_id) {
            Ok(prev_blob_id) => prev_blob_id,
            Err(_) => {
                let msg = format!("update group({}) failed for invalid prev-blob-id {}", group_id, prev_blob_id);
                log::error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }
        };
        group.set_prev_blob_id(Some(prev_blob_id));
    }

    if group.admins().is_empty() {
        let msg = format!("update group({}) failed for no administrators", group_id);
        log::error!("{}", msg);
        return Err(BuckyError::new(BuckyErrorCode::InvalidInput, msg));        
    }

    group.body_mut().as_mut().unwrap().set_update_time(bucky_time_now());

    Ok(())
}
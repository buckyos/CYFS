use clap::{ArgMatches, App, SubCommand, Arg};
use crate::desc;
use crate::util::{get_objids_from_matches, get_deviceids_from_matches};
use std::path::{Path};
use log::*;
use cyfs_base::{Area, FileEncoder, NamedObject, ObjectDesc, AnyNamedObject, FileDecoder, PublicKeyRef, DeviceCategory, ObjectId, RsaCPUObjectSigner, sign_and_set_named_object_desc, SignatureSource, SIGNATURE_SOURCE_REFINDEX_SELF, SIGNATURE_SOURCE_REFINDEX_OWNER};
use std::str::FromStr;
use std::io::Write;
use crate::desc::create_people_desc;


pub fn create_subcommand<'a, 'b>() -> App<'a, 'b> {
    let id_file_arg = Arg::with_name("id_file").long("idfile").takes_value(true).help("write object id to file");
    let save_path = Arg::with_name("save_path").long("savepath").takes_value(true).help("save file path");
    SubCommand::with_name("create").about("create desc")
        .subcommand(SubCommand::with_name("people").about("create people desc and sec")
            .arg(Arg::with_name("owner").long("owner").short("o").takes_value(true)
                .help("people's owner id"))
            .arg(Arg::with_name("ood_list").long("oodlist").short("l").takes_value(true).value_delimiter(";")
                .help("oods in people"))
            .arg(Arg::with_name("pktype").long("pktype").short("p").default_value("rsa1024")
                .required(true).possible_values(&["rsa1024", "rsa2048", "rsa3072", "secp"])
                .help("private key type"))
            .arg(Arg::with_name("area").long("area").short("a").takes_value(true)
                .help("Object area info, if not set,will calc base ip. format [county:carrier:city:inner]"))
            .arg(id_file_arg.clone()).arg(save_path.clone()))
        .subcommand(SubCommand::with_name("sgroup").about("create sgroup desc")
            .arg(Arg::with_name("threshold").long("threshold").default_value("1").required(true)
                .help("threshold in simple group"))
            .arg(Arg::with_name("members").long("members").short("m").value_delimiter(";")
                .help("members in simple group"))
            .arg(Arg::with_name("ood_list").long("oodlist").short("l").value_delimiter(";")
                .help("oods in group"))
            .arg(Arg::with_name("owners").long("owners").short("o").value_delimiter(";")
                .help("group owners, must have desc file in same path"))
            .arg(id_file_arg.clone()).arg(save_path.clone()))
        .subcommand(SubCommand::with_name("device").about("create device desc and sec")
            .arg(Arg::with_name("area").long("area").short("a").takes_value(true)
                .help("Object area info, if not set,will calc base ip. format [county:carrier:city:inner]"))
            .arg(Arg::with_name("pktype").long("pktype").short("p").default_value("rsa1024")
                .required(true).possible_values(&["rsa1024", "rsa2048", "rsa3072", "secp"])
                .help("private key type"))
            .arg(Arg::with_name("deviceid").long("deviceid").short("d").takes_value(true).validator(|v|{
                return if v.len() > 0 && v.len() <= 16 { Ok(()) } else { Err(String::from("deviceid length must between 0 and 16")) }
            }).required_ifs(&[("type", "device")])
                .help("input uniqueid"))
            .arg(Arg::with_name("category").short("c").long("category").takes_value(true)
                .possible_values(&["ood", "mobile", "pc", "server", "browser"]).required_if("type", "device")
                .help("device category: ood/mobile/pc/server/browser"))
            .arg(Arg::with_name("owner").long("owner").short("o").takes_value(true)
                .help("people's owner id"))
            .arg(Arg::with_name("eps").long("eps").short("e").value_delimiter(";")
                .help("Endpoint list,first char identify the ip is wan or internal"))
            .arg(Arg::with_name("snlist").long("snlist").short("s").value_delimiter(";")
                .help("peer sn peerid"))
            .arg(id_file_arg.clone()).arg(save_path.clone()))
        .subcommand(SubCommand::with_name("runtime").about("create people, ood, device desc for cyfs runtime")
            .arg(Arg::with_name("area").long("area").short("a").takes_value(true)
                .help("Object area info, if not set,will calc base ip. format [county:carrier:city:inner]"))
            .arg(Arg::with_name("pktype").long("pktype").short("p").default_value("rsa1024")
                .required(true).possible_values(&["rsa1024", "rsa2048", "rsa3072", "secp"])
                .help("private key type"))
            .arg(save_path.clone()))
}

fn write_id_file(matches: &ArgMatches, id: &ObjectId) {
    if let Some(file) = matches.value_of("id_file") {
        let mut file = std::fs::File::create(file).unwrap();
        file.write(id.to_string().as_ref()).unwrap();
    }
}

fn get_area(matches: &ArgMatches) -> Option<Area> {
    matches.value_of("area").map(|str_area| {
        match Area::from_str(str_area) {
            Ok(area) => area,
            Err(_) => {
                error!("decode area from {} fail, use default", str_area);
                Area::default()
            },
        }
    })
}

fn get_key_bits(matches: &ArgMatches) -> usize {
    match matches.value_of("pktype").unwrap() {
        "rsa1024" => 1024,
        "rsa2048" => 2048,
        "rsa3072" => 3072,
        "secp" => 1,
        _ => 0
    }
}

pub async fn create_desc(matches: &ArgMatches<'_>) {
    match matches.subcommand() {
        ("device", Some(matches)) => {
            let owner = matches.value_of("owner").map(|str| {
                ObjectId::from_str(str).unwrap()
            });
            let sn_list = get_deviceids_from_matches(matches,"snlist").unwrap_or(vec![]);
            let eps = matches.values_of_lossy("eps").unwrap_or(vec![]);

            let str_unique_id = matches.value_of("deviceid").unwrap();

            let key_bits = get_key_bits(matches);

            if key_bits == 0 {
                error!("invalid pktype");
                return;
            }

            let category = match matches.value_of("category").unwrap() {
                "ood" => DeviceCategory::OOD,
                "mobile" => DeviceCategory::AndroidMobile,
                "pc" => DeviceCategory::PC,
                "server" => DeviceCategory::Server,
                "browser" => DeviceCategory::Browser,
                _ => {unreachable!()}
            };

            let area = get_area(matches);

            let save_path = matches.value_of("save_path").unwrap_or("").to_owned();
            if let Some((device, _)) = desc::create_device_desc(area, category, key_bits, str_unique_id, owner, eps, sn_list, Some(save_path)) {
                write_id_file(matches, &device.desc().calculate_id());
            }
            return;
        }
        ("sgroup", Some(matches)) => {
            match matches.value_of("threshold").unwrap().parse::<u8>() {
                Ok(threshold) => {
                    let owners = if let Some(strs) = matches.values_of_lossy("owners") {
                        let mut owners = vec![];
                        for str in &strs {
                            if let Ok((obj, _)) = AnyNamedObject::decode_from_file(&Path::new(str).with_extension("desc"), &mut vec![]) {
                                if let Some(pk) = obj.public_key() {
                                    match pk {
                                        PublicKeyRef::Single(pk) => {owners.push(pk.clone())}
                                        PublicKeyRef::MN((_, pks)) => {
                                            owners.append(&mut pks.clone())
                                        }
                                    }
                                } else {
                                    warn!("desc {} not have pubkey, ignore", str);
                                }
                            } else {
                                error!("decode desc file {} fail", str);
                            }
                        }
                        owners
                    } else {
                        vec![]
                    };

                    if threshold as usize > owners.len() {
                        error!("threshold must small owners count, detail info use --help");
                        return;
                    }

                    let group_desc = desc::create_simple_group_desc(threshold
                                                                    , owners
                                                                    , get_objids_from_matches(matches, "members")
                                                                    , get_deviceids_from_matches(matches, "ood_list"));

                    let groupid = group_desc.desc().calculate_id();
                    let desc_file = Path::new(matches.value_of("save_path").unwrap_or("")).join(&groupid.to_string()).with_extension("desc");
                    if let Err(e) = group_desc.encode_to_file(&desc_file, true) {
                        error!("write imple group desc file failed, err {}", e);
                    } else {
                        info!("write simple group desc file succ to {}", desc_file.display());
                        write_id_file(matches, &groupid);
                    };
                },
                Err(_e) => {
                    error!("threshold must number, detail info use --help");
                    return;
                }
            }
        }
        ("people", Some(matches)) => {
            let owner_id = matches.value_of("owner").map(|str| {
                ObjectId::from_str(str).unwrap()
            });
            let ood_list = get_deviceids_from_matches(matches, "ood_list").unwrap_or(vec![]);
            let key_bits = get_key_bits(matches);
            let area = get_area(matches);
            let (people, secret) = create_people_desc(area, key_bits, owner_id, ood_list);
            let objid = people.desc().calculate_id();
            let file_path = Path::new(matches.value_of("save_path").unwrap_or("")).join(&objid.to_string()).with_extension("desc");
            if let Err(e) = people.encode_to_file(&file_path, true) {
                error!("write people file failed, err {}", e);
            } else {
                info!("write people desc file succ to {}", file_path.display());
                write_id_file(matches, &objid);
            };

            if let Err(e) = secret.encode_to_file(&file_path.with_extension("sec"), true) {
                error!("write people secret failed, err {}", e);
            }
        }
        ("runtime", Some(matches)) => {
            let key_bits = get_key_bits(matches);
            let area = get_area(matches);

            // 先创建people
            let (mut people, people_sec) = create_people_desc(area.clone(), key_bits, None, vec![]);
            let people_id = people.desc().calculate_id();

            // 再创建ood，使用people为owner
            let (mut ood_desc, ood_sec) = desc::create_device_desc(area.clone(), DeviceCategory::OOD, key_bits, "ood", Some(people_id.clone())
                                                                   , vec![], vec![], None).unwrap();

            // 修改people的ood_list
            people.ood_list_mut().push(ood_desc.desc().device_id());
            // 再创建client，使用people为owner
            let (mut client_desc, client_sec) = desc::create_device_desc(area, DeviceCategory::PC, key_bits, "client", Some(people_id.clone())
                                                                         , vec![], vec![], None).unwrap();

            let signer = RsaCPUObjectSigner::new(people_sec.public(), people_sec.clone());
            // 给desc签名
            sign_and_set_named_object_desc(&signer, &mut people, &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_SELF)).await.unwrap();
            sign_and_set_named_object_desc(&signer, &mut ood_desc, &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER)).await.unwrap();
            sign_and_set_named_object_desc(&signer, &mut client_desc, &SignatureSource::RefIndex(SIGNATURE_SOURCE_REFINDEX_OWNER)).await.unwrap();
            let file_path = Path::new(matches.value_of("save_path").unwrap_or(""));
            // 存储这些对象
            let mut postfix = String::from("");
            if Path::new(&file_path.join(format!("device{}", &postfix)).with_extension("desc")).exists() {
                postfix = chrono::Local::now().format("-%F-%H-%M-%S").to_string();
            }
            let people_file = file_path.join(format!("people{}", &postfix));
            if let Err(e) = people.encode_to_file(&people_file.with_extension("desc"), false) {
                error!("write people desc failed, err {}", e);
            }

            if let Err(e) = people_sec.encode_to_file(&people_file.with_extension("sec"), false) {
                error!("write people sec failed, err {}", e);
            }

            let ood_file = file_path.join(format!("ood{}", &postfix));
            if let Err(e) = ood_desc.encode_to_file(&ood_file.with_extension("desc"), false) {
                error!("write ood desc failed, err {}", e);
            }

            if let Err(e) = ood_sec.encode_to_file(&ood_file.with_extension("sec"), false) {
                error!("write ood sec failed, err {}", e);
            }

            let client_file = file_path.join(format!("device{}", &postfix));
            if let Err(e) = client_desc.encode_to_file(&client_file.with_extension("desc"), false) {
                error!("write client desc failed, err {}", e);
            }

            if let Err(e) = client_sec.encode_to_file(&client_file.with_extension("sec"), false) {
                error!("write client sec failed, err {}", e);
            }

        }
        v @ _ => {
            error!("not support create type {}", v.0);
            return;
        }
    }
}
use clap::{App, Arg, ArgMatches, SubCommand};
use cyfs_base::{
    AnyNamedObject, ChunkList, Dir, DirBodyContent, File, FileDecoder, InnerNode, NDNObjectInfo,
    NamedObject, ObjectDesc, RawEncode, RawFrom, SignatureSource, SingleKeyObjectDesc,
    StandardObject,
};
use cyfs_core::{AppList, AppListObj, AppStatusObj, CoreObjectType, DecApp, DecAppObj};
use std::convert::TryFrom;
use std::path::Path;

pub fn show_desc_subcommand<'a, 'b>() -> App<'a, 'b> {
    SubCommand::with_name("show")
        .about("show desc")
        .arg(
            Arg::with_name("desc_file")
                .required(true)
                .index(1)
                .help("desc file to show"),
        )
        .arg(
            Arg::with_name("show_endpoint")
                .short("e")
                .long("endpoint")
                .help("show endpoint"),
        )
        .arg(
            Arg::with_name("show_snlist")
                .short("s")
                .long("snlist")
                .help("show snlist"),
        )
        .arg(
            Arg::with_name("show_owners")
                .short("o")
                .long("owner")
                .help("show owner"),
        )
        .arg(
            Arg::with_name("show_constinfo")
                .short("c")
                .long("constinfo")
                .help("show constinfo(uniqueid and pubkey in hex)"),
        )
        .arg(
            Arg::with_name("show_members")
                .short("m")
                .long("member")
                .help("show members in simple group"),
        )
        .arg(
            Arg::with_name("show_oodlist")
                .short("l")
                .long("ood_list")
                .help("show ood_list in people"),
        )
        .arg(
            Arg::with_name("all")
                .short("a")
                .long("all")
                .help("show all printable infos"),
        )
        .arg(
            Arg::with_name("show_signs")
                .long("signs")
                .help("show signs"),
        )
        .arg(
            Arg::with_name("raw")
                .short("r")
                .long("raw")
                .help("show raw hex"),
        )
}

fn format_sign_source(source: &SignatureSource) -> String {
    match source {
        SignatureSource::RefIndex(i) => {
            format!("ref: {}", i)
        }
        SignatureSource::Object(link) => {
            format!("object: {}", &link.obj_id)
        }
        SignatureSource::Key(key) => {
            format!("public key {:?}", key)
        }
    }
}

fn show_file(file: &File, matches: &ArgMatches) {
    println!("desc type: File");
    if matches.is_present("all") {
        match file.body().as_ref().unwrap().content().chunk_list() {
            ChunkList::ChunkInList(list) => {
                print!("chunks: [");
                for chunk in list {
                    print!("{}, ", chunk);
                }
                println!("]");
            }
            ChunkList::ChunkInFile(fileid) => {
                println!("chunks: file {}", fileid);
            }
            ChunkList::ChunkInBundle(bundle) => {
                print!("chunks: [");
                for chunk in bundle.chunk_list() {
                    print!("{}, ", chunk);
                }
                println!("]");
            }
        }
    }
}

fn show_dir(dir: &Dir, matches: &ArgMatches) {
    println!("desc type: Dir");
    if matches.is_present("all") {
        match dir.desc().content().obj_list() {
            NDNObjectInfo::ObjList(entrys) => {
                for (path, info) in &entrys.object_map {
                    println!("path: {}", path);
                    match info.node() {
                        InnerNode::ObjId(id) => {
                            let obj = match dir.body_expect("").content() {
                                DirBodyContent::Chunk(_) => {
                                    println!("\t dir in chunk not support");
                                    None
                                }
                                DirBodyContent::ObjList(objs) => {
                                    if let Some(buf) = objs.get(id) {
                                        Some(StandardObject::clone_from_slice(buf).unwrap())
                                    } else {
                                        None
                                    }
                                }
                            };
                            if let Some(obj) = obj {
                                match obj {
                                    StandardObject::Dir(dir) => {
                                        println!("\tdir: {}", id);
                                        show_dir(&dir, matches);
                                    }
                                    StandardObject::File(file) => {
                                        println!("\tfile: {}", id);
                                        show_file(&file, matches);
                                    }
                                    _ => {}
                                }
                            } else {
                                println!("error: cannot find obj {}!!!", id);
                            }
                        }
                        InnerNode::Chunk(id) => {
                            println!("\tchunk: {}", id);
                        }
                        InnerNode::IndexInParentChunk(_, _) => {}
                    }
                }
            }
            NDNObjectInfo::Chunk(chunk) => {
                println!("content: chunk {}", chunk);
            }
        }
    }
}

pub fn show_desc(matches: &ArgMatches) {
    let path = Path::new(matches.value_of("desc_file").unwrap());
    let mut file_buf = vec![];
    match AnyNamedObject::decode_from_file(path, &mut file_buf) {
        Ok((desc, _)) => {
            println!("objectid: {}", desc.calculate_id());
            if matches.is_present("all") || matches.is_present("show_owners") {
                if let Some(owner) = desc.owner() {
                    println!("owner: {}", owner);
                } else {
                    println!("owner: None");
                }
            }

            if matches.is_present("raw") {
                println!("raw: {}", hex::encode(&file_buf));
            }

            if matches.is_present("all") || matches.is_present("show_signs") {
                if let Some(signs) = desc.signs() {
                    if let Some(signs) = signs.desc_signs() {
                        println!("have {} desc signs: ", signs.len());
                        for sign in signs {
                            println!(
                                "\t sign: {}, time {}",
                                format_sign_source(sign.sign_source()),
                                sign.sign_time()
                            )
                        }
                    }

                    if let Some(signs) = signs.body_signs() {
                        println!("have {} body signs: ", signs.len());
                        for sign in signs {
                            println!(
                                "\t sign: {}, time {}",
                                format_sign_source(sign.sign_source()),
                                sign.sign_time()
                            )
                        }
                    }
                }
            }

            if matches.is_present("all") {
                println!("desc time: {}", desc.create_time());
                println!("body time: {}", desc.get_update_time());
            }

            match &desc {
                AnyNamedObject::Standard(standard) => match standard {
                    StandardObject::Device(p) => {
                        println!("desc type: device");
                        if matches.is_present("all") || matches.is_present("show_endpoint") {
                            print!("endpoint: [");
                            for endpoint in p.body().as_ref().unwrap().content().endpoints() {
                                print!("{}, ", endpoint);
                            }
                            println!("]");
                        }
                        if matches.is_present("all") || matches.is_present("show_snlist") {
                            print!("snlist: [");
                            for sn in p.body().as_ref().unwrap().content().sn_list() {
                                print!("{}, ", sn);
                            }
                            println!("]");
                        }
                        if matches.is_present("all") || matches.is_present("show_constinfo") {
                            println!("uniqueid: {}", hex::encode(p.desc().unique_id().as_slice()));
                            let pubkey = p.desc().public_key();
                            let mut buf = vec![];
                            buf.resize(pubkey.raw_measure(&None).unwrap(), 0);
                            pubkey.raw_encode(&mut buf, &None).expect("encode pubkey err");
                            println!("pubkey: {}", hex::encode(&buf));
                            println!("createtime: {}", p.desc().create_time());
                            println!("device catelogy: {}", p.category().unwrap())
                        }
                    }
                    StandardObject::UnionAccount(ua) => {
                        println!("desc type: UnionAccount");
                        println!(
                            "{} <=> {}",
                            ua.desc().content().left(),
                            ua.desc().content().right()
                        );
                    }
                    StandardObject::SimpleGroup(g) => {
                        println!("desc type: Group");
                        if matches.is_present("all") || matches.is_present("show_members") {
                            print!("members: [");
                            for owner in g.body().as_ref().unwrap().content().members() {
                                print!("{}, ", owner);
                            }
                            println!("]");
                        }
                    }
                    StandardObject::File(f) => {
                        show_file(f, matches);
                    }
                    StandardObject::Dir(p) => {
                        show_dir(p, matches);
                    }
                    StandardObject::People(p) => {
                        println!("desc type: People");
                        if matches.is_present("all") || matches.is_present("show_oodlist") {
                            print!("ood_list: [");
                            for ood in p.body().as_ref().unwrap().content().ood_list() {
                                print!("{}, ", ood);
                            }
                            println!("]");
                        }

                        if matches.is_present("all") {
                            println!(
                                "name: {}",
                                p.body()
                                    .as_ref()
                                    .unwrap()
                                    .content()
                                    .name()
                                    .unwrap_or("None")
                            );
                            if let Some(id) = p.body().as_ref().unwrap().content().icon() {
                                println!("icon: {}", id);
                            } else {
                                println!("icon: None");
                            }
                        }
                    }
                    v @ _ => {
                        println!("unsupport type {}", v.obj_type_code() as u8);
                    }
                },
                AnyNamedObject::Core(core) => match CoreObjectType::from(core.desc().obj_type()) {
                    CoreObjectType::DecApp => {
                        let app = DecApp::try_from(core.clone()).unwrap();
                        println!("desc type: Dec App");
                        println!("name {}", app.name());
                        for (ver, source) in app.source() {
                            println!("app have source: {} : {}", ver, source);
                        }
                    }
                    CoreObjectType::AppList => {
                        let list = AppList::try_from(core.clone()).unwrap();
                        println!("desc type: App List");
                        for (id, status) in list.app_list() {
                            println!(
                                "app {}: ver {}, start {}",
                                &id,
                                status.version(),
                                status.status()
                            )
                        }
                    }
                    v @ _ => {
                        println!("unsupport core obj type {}", v as u16);
                    }
                },
                AnyNamedObject::DECApp(dec) => {
                    println!("dec app obj, type code {}", dec.desc().obj_type());
                }
            }
        }
        Err(e) => {
            println!("read desc from file {} failed, err {}", path.display(), e);
        }
    }
}

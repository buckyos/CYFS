#[macro_use]
extern crate log;

use clap::{App, Arg};
use std::path::{Path, PathBuf};
use std::error::Error;
use simple_logger::SimpleLogger;
use std::str::FromStr;

mod zip_package;
use crate::zip_package::ZipPackage;

// 为整个dir作为一个zip计算hash
fn append_package_hash(pkg: &mut ZipPackage, dir: &Path) -> Result<(), Box<dyn Error>> {
    info!("found target folder {}", dir.display());

    let mut zip = ZipPackage::new();
    zip.load(dir);

    let name = PathBuf::from_str(".hash").unwrap();

    let hash = zip.calc_hash()?;
    info!("dir folder zip hash: {} -> {}", dir.display(), hash);

    pkg.append_file(&name, hash.as_bytes())?;

    Ok(())
}

/*
// 为一个dir下面所有的子目录计算hash
fn append_package_hash(pkg: &mut ZipPackage, dir: &Path) -> Result<(), Box<dyn Error>> {
    let is_ignore = |entry: &DirEntry| -> bool {
        entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with("."))
            .unwrap_or(false)
    };

    info!("will calc package hashes! dir={}", dir.display());

    let walker = WalkDir::new(dir).max_depth(1).into_iter();
    for entry in walker.filter_entry(|e| !is_ignore(e)) {
        if let Err(e) = entry {
            error!("find top level dir error! e={}", e);
            continue;
        }

        let entry = entry.unwrap();
        if !entry.file_type().is_dir() {
            continue;
        }

        let path = entry.path();
        if path == dir {
            continue;
        }

        info!("found target folder {}", path.display());

        let mut zip = ZipPackage::new();
        zip.load(entry.path());

        let name = path.strip_prefix(&dir).unwrap().join(".hash");

        let hash = zip.calc_hash()?;
        pkg.append_file(&name, hash.as_bytes())?;
    }

    Ok(())
}
*/
fn main() {
    SimpleLogger::new().with_level(log::LevelFilter::Debug).init().unwrap();

    let matches = App::new("ffs pack tools")
        .version(cyfs_base::get_version())
        .about("ood service pack tools for ffs system")
        .author("CYFS <dev@cyfs.com>")
        .arg(
            Arg::with_name("dir")
                .short("d")
                .long("dir")
                .takes_value(true)
                .help("Service folder had package.cfg included at root"),
        )
        .arg(
            Arg::with_name("file")
                .short("f")
                .long("file")
                .takes_value(true)
                .help("Target zip file, default to [folder_name].zip"),
        )
        .get_matches();

    let dir = matches.value_of("dir").unwrap();
    let dir = dir.trim_end_matches("/").trim_end_matches("\\");
    let dir_path = Path::new(dir);
    if !dir_path.is_dir() {
        error!(
            "dir folder not exists or valid dir! path={}",
            dir_path.display()
        );
        std::process::exit(-1);
    }
    info!("will pack dir: {}", dir_path.display());
    /*
    暂时屏蔽package.cfg的检测
    let package_cfg_path = dir_path.join("package.cfg");
    if !package_cfg_path.is_file() {
        error!("package.cfg not found! path={}", package_cfg_path.display());
        std::process::exit(-1);
    }

    info!("found package.cfg: {}", package_cfg_path.display());
    */

    let dir_name = dir_path.file_name().unwrap().to_str().unwrap();
    let default_file = dir_path.join(format!("..{}{}.zip", std::path::MAIN_SEPARATOR, dir_name));
    let file = matches
        .value_of("file")
        .unwrap_or(default_file.to_str().unwrap());

    info!("will pack to file: {}", file);

    let mut zip = ZipPackage::new();
    zip.load(&dir_path);

    if let Err(e) = zip.begin_zip(file) {
        error!("zip package error! err={}", e);
        std::process::exit(-1);
    }

    if let Err(e) = append_package_hash(&mut zip, &dir_path) {
        error!("append package hash error! err={}", e);
        std::process::exit(-1);
    }

    let ret = zip.finish_zip();
    if let Err(e) = ret {
        error!("finish zip to target file failed! err={}", e);
        std::process::exit(-1);
    }

    info!("build zip package success! target={}", file);

    std::process::exit(0);
}

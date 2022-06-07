extern crate walkdir;
use sha2::{Digest, Sha256};
use std::error::Error;

use std::fs::File;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};
use zip;
use std::fmt::Write;
use std::io::{Read};


fn path_to_string(path: &std::path::Path) -> String {
    let mut path_str = String::new();
    for component in path.components() {
        if let std::path::Component::Normal(os_str) = component {
            if !path_str.is_empty() {
                path_str.push('/');
            }
            path_str.push_str(&*os_str.to_string_lossy());
        }
    }
    path_str
}
pub struct ZipPackage {
    src_dir: PathBuf,
    all: Vec<PathBuf>,
    hash: Option<String>,
    zip_file: Option<zip::ZipWriter<std::fs::File>>,
}

impl ZipPackage {
    pub fn new() -> ZipPackage {
        ZipPackage {
            src_dir: PathBuf::from(""),
            all: Vec::new(),
            hash: None,
            zip_file: None,
        }
    }

    pub fn load(&mut self, dir: &Path) {
        assert!(self.all.is_empty());

        self.src_dir = dir.to_owned();

        let is_ignore = |entry: &DirEntry| -> bool {
            entry
                .file_name()
                .to_str()
                .map(|s| s.starts_with("."))
                .unwrap_or(false)
        };
        let walker = WalkDir::new(dir).into_iter();
        for entry in walker.filter_entry(|e| !is_ignore(e)) {
            let entry = entry.unwrap();
            if entry.file_type().is_dir() {
                continue;
            }

            let path = entry.path();
            //debug!("{}", entry.path().display());
            self.all.push(path.to_path_buf());
        }
        self.all.sort();
    }

    pub fn calc_hash(&mut self) -> Result<String, Box<dyn Error>> {
        let mut hasher = Sha256::new();
        for path in &self.all {
            let ret = File::open(&path);
            if let Err(e) = ret {
                let msg = format!("open file error! file={}, err={}", path.display(), e);
                error!("{}", msg);
                return Err(Box::<dyn Error>::from(msg));
            }
            let mut file = ret.unwrap();
            let ret = std::io::copy(&mut file, &mut hasher);
            if let Err(e) = ret {
                let msg = format!("read file error! file={}, err={}", path.display(), e);
                error!("{}", msg);
                return Err(Box::<dyn Error>::from(msg));
            }
        }
        let hex = hasher.result();
        let mut s = String::new();
        for &byte in hex.as_slice() {
            write!(&mut s, "{:X}", byte).expect("Unable to format hex string");
        }

        self.hash = Some(s.clone());

        Ok(s)
    }

    pub fn begin_zip(&mut self, dest_file: &str) -> Result<(), Box<dyn Error>> {
        assert!(self.zip_file.is_none());

        use std::io::Write;

        let target_file = File::create(dest_file).unwrap();

        let mut zip = zip::ZipWriter::new(target_file);

        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Bzip2);
        let mut buffer = Vec::new();
        for path in &self.all {
            let name = path.strip_prefix(&self.src_dir).unwrap();

            info!(
                "adding file to zip {} => {} ...",
                path.display(),
                name.display()
            );

            /*
            TODO 更新最后修改时间
            let metadata = std::fs::metadata(path)?;

            let opt;
            if let Ok(time) = metadata.modified() {
                opt = options.last_modified_time(time);
            } else {
                println!("Not supported on this platform");
                opt = options;
            }
            */

            let opt;
            #[cfg(windows)] 
            {
                opt = options;
            }

            #[cfg(not(windows))]
            {
                use std::os::unix::fs::PermissionsExt;
                let metadata = std::fs::metadata(path)?;
                //info!("mode {}", metadata.permissions().mode());
                opt = options.unix_permissions(metadata.permissions().mode());
            }

            // name.to_string_lossy()
            zip.start_file(path_to_string(name), opt)?;

            let ret = File::open(path);
            if let Err(e) = ret {
                return Err(Box::new(e));
            }

            let mut f = ret.unwrap();
            f.read_to_end(&mut buffer)?;
            zip.write_all(&*buffer)?;

            buffer.clear();
        }
        self.zip_file = Some(zip);

        Ok(())
    }

    pub fn append_pkg_hash(&mut self) -> Result<(), Box<dyn Error>> {
        assert!(self.zip_file.is_some());

        if self.hash.is_none() {
            if let Err(e) = self.calc_hash() {
                error!("calc hash error! err={}", e);
                return Err(e);
            }
        }

        // 添加.hash文件
        {
            let options = zip::write::FileOptions::default()
                .compression_method(zip::CompressionMethod::Bzip2);
            let name = Path::new(".hash");

            info!(
                "adding .hash file to zip {} = {} ...",
                name.display(),
                self.hash.as_ref().unwrap()
            );

            let zip = self.zip_file.as_mut().unwrap();
            zip.start_file(path_to_string(name), options)?;

            use std::io::Write;
            zip.write_all(self.hash.as_ref().unwrap().as_bytes())?;
        }

        Ok(())
    }

    pub fn append_file(&mut self, path: &Path, bytes: &[u8]) -> Result<(), Box<dyn Error>> {
        assert!(self.zip_file.is_some());

        let options =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Bzip2);

        info!("adding file to zip {}...", path.display(),);

        let zip = self.zip_file.as_mut().unwrap();
        zip.start_file(path_to_string(path), options)?;

        use std::io::Write;
        zip.write_all(bytes)?;

        Ok(())
    }

    pub fn finish_zip(&mut self)  -> Result<(), Box<dyn Error>> {
        assert!(self.zip_file.is_some());

        let mut zip = self.zip_file.take().unwrap();

        // Optionally finish the zip. (this is also done on drop)
        zip.finish()?;

        Ok(())
    }
}

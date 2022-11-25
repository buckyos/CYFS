use std::collections::HashMap;
use std::fs;
// use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use cyfs_base::{BuckyResult, BuckyError, BuckyErrorCode, NamedObject, ObjectDesc};
use cyfs_base::{RawConvertTo, RawFrom, bucky_time_now};
use cyfs_base::{ObjectId, DeviceId, Device, NDNObjectInfo, Dir, Attributes, NDNObjectList, InnerNode, InnerNodeInfo};

struct DirObjectsSyncLoader {
    roots: Vec<PathBuf>,
    objects: Vec<(PathBuf, Vec<u8>)>,
}

impl DirObjectsSyncLoader {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![root.into()],
            objects: Vec::new(),
        }
    }

    pub fn into_objects(self) -> Vec<(PathBuf, Vec<u8>)> {
        self.objects
    }

    pub fn load(&mut self) {
        let mut i = 0;
        loop {
            if i >= self.roots.len() {
                break;
            }

            let root = self.roots[i].clone();
            let _ = self.scan_root(&root);

            i += 1;
        }
    }

    fn scan_root(&mut self, root: &Path) -> BuckyResult<()> {
        if !root.is_dir() {
            return Ok(());
        }

        let mut entries = fs::read_dir(root).map_err(|e| {
            println!("cargo:warning={}", format!(
                "read object dir failed! dir={}, {}",
                root.display(),
                e
            ));
            e
        })?;

        while let Some(res) = entries.next() {
            let entry = res.map_err(|e| {
                println!("cargo:warning={}", format!("read entry error: {}", e));
                e
            })?;

            let file_path = root.join(entry.file_name());
            if file_path.is_dir() {
                self.roots.push(file_path);
                continue;
            }

            if !file_path.is_file() {
                println!("cargo:warning={}", format!("path is not file: {}", file_path.display()));
                continue;
            }

            if !Self::is_desc_file(&file_path) {
                println!("cargo:warning={}", format!("not desc file: {}", file_path.display()));
                continue;
            }

            if let Ok(ret) = self.load_file(&file_path) {
                self.objects.push((file_path, ret));
            }
        }

        Ok(())
    }

    fn is_desc_file(file_path: &Path) -> bool {
        match file_path.extension() {
            Some(ext) => {
                let ext = ext.to_string_lossy();

                #[cfg(windows)]
                    let ext = ext.to_lowercase();

                if ext == "desc" {
                    true
                } else {
                    false
                }
            }
            None => false,
        }
    }

    fn load_file(&self, file: &Path) -> BuckyResult<Vec<u8>> {
        let buf = fs::read(file).map_err(|e| {
            println!("cargo:warning={}", format!("load object from file failed! file={}, {}", file.display(), e));
            e
        })?;

        Ok(buf)
    }
}

fn load_device_objects_list(root: &Path) -> Vec<(DeviceId, Device)> {
    if !root.is_dir() {
        return vec![];
    }

    let mut loader = DirObjectsSyncLoader::new(root);
    loader.load();

    let objects = loader.into_objects();
    let mut result = Vec::with_capacity(objects.len());
    for (file_path, data) in objects {
        match Device::clone_from_slice(&data) {
            Ok(device) => {
                let id = device.desc().device_id();
                result.push((id, device));
            }
            Err(e) => {
                println!("cargo:warning={}", format!(
                    "invalid local device object: file={}, {}",
                    file_path.display(),
                    e
                ));
            }
        }
    }

    result
}

struct SNDirGenerator;

impl SNDirGenerator {
    pub fn gen_from_dir(owner_id: &Option<ObjectId>, root: &Path) -> BuckyResult<Dir> {
        let list = load_device_objects_list(root);
        if list.is_empty() {
            let msg = format!("sn device folder is empty! dir={}", root.display());
            println!("cargo:warning={}", format!("{}", msg));
            return Err(BuckyError::new(BuckyErrorCode::InvalidData, msg));
        }

        Self::gen_from_list(owner_id, list)
    }

    pub fn gen_from_list(
        owner_id: &Option<ObjectId>,
        list: Vec<(DeviceId, Device)>,
    ) -> BuckyResult<Dir> {
        let mut object_map = HashMap::new();
        let mut body_map = HashMap::new();
        for (id, sn) in list {
            let inner_node = InnerNodeInfo::new(
                Attributes::default(),
                InnerNode::ObjId(id.object_id().clone()),
            );

            let path = format!("list/{}", id);
            if let Some(_prev) = object_map.insert(path, inner_node) {
                let msg = format!("sn device item already exists in desc! id={}", id);
                println!("cargo:warning={}", format!("{}", msg));
                return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
            }

            let buf = sn.to_vec()?;
            if let Some(_prev) = body_map.insert(id.object_id().clone(), buf) {
                let msg = format!("sn device item already exists in body! id={}", id);
                println!("cargo:warning={}", format!("{}", msg));
                return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
            }
        }

        let list = NDNObjectList {
            parent_chunk: None,
            object_map,
        };

        let attr = Attributes::new(0);
        let builder = Dir::new(attr, NDNObjectInfo::ObjList(list.clone()), body_map);
        let dir = builder
            .option_owner(owner_id.to_owned())
            .no_create_time()
            .update_time(bucky_time_now())
            .build();
        let dir_id = dir.desc().calculate_id();
        Ok(dir)
    }
}

fn main() {
    println!("cargo:rerun-if-changed=peers/nightly-sn");
    println!("cargo:rerun-if-changed=peers/beta-sn");
    let owner = cyfs_base::ObjectId::from_str("5aSixgLzAmyR5QbQibWFkrkNbBLagfawmK3pbdaYqyt6").unwrap();
    let nightly_sns = SNDirGenerator::gen_from_dir(&Some(owner), Path::new("peers/nightly-sn")).unwrap();
    // let mut nightly_sn_raw = vec![];
    //std::fs::File::open("peers/nightly-sn.desc").unwrap().read_to_end(&mut nightly_sn_raw).unwrap();
    println!("cargo:rustc-env=NIGHTLY_SN_RAW={}", nightly_sns.to_hex().unwrap());

    //let mut beta_sn_raw = vec![];
    let beta_sns = SNDirGenerator::gen_from_dir(&Some(owner), Path::new("peers/nightly-sn")).unwrap();
    //std::fs::File::open("peers/beta-sn.desc").unwrap().read_to_end(&mut beta_sn_raw).unwrap();
    println!("cargo:rustc-env=BETA_SN_RAW={}", beta_sns.to_hex().unwrap());
}
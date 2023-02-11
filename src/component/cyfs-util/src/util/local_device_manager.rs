use cyfs_base::{
    BuckyError, BuckyErrorCode, BuckyResult, Device, FileDecoder, NamedObject, PrivateKey,
    StandardObject,
};

use std::collections::{hash_map::Entry, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;

#[derive(Clone, Debug)]
pub struct DeviceInfo {
    pub device: Device,

    pub private_key: Option<PrivateKey>,
}

// 用以管理本地磁盘上的device文件和密钥
struct LocalDeviceManagerImpl {
    root: PathBuf,
    list: HashMap<String, DeviceInfo>,
}

impl LocalDeviceManagerImpl {
    fn new() -> Self {
        let root = crate::get_cyfs_root_path().join("etc").join("desc");

        Self {
            root,
            list: HashMap::new(),
        }
    }

    pub fn get_root(&self) -> PathBuf {
        self.root.clone()
    }

    // 切换root
    pub fn set_root(&mut self, dir: &Path) {
        if self.root != dir {
            info!(
                "change ood desc dir: {} -> {}",
                self.root.display(),
                dir.display()
            );
            self.root = dir.to_owned();
            self.list.clear();
        }
    }

    // 清空缓存
    pub fn clear_cache(&mut self) {
        self.list.clear();
    }

    // 直接添加一个device
    pub fn add(&mut self, name: &str, device: DeviceInfo) -> BuckyResult<()> {
        match self.list.entry(name.to_owned()) {
            Entry::Occupied(_o) => {
                error!(
                    "direct add new device but already exists! id={}, device={}",
                    name,
                    device.device.desc().device_id()
                );

                Err(BuckyError::from(BuckyErrorCode::AlreadyExists))
            }
            Entry::Vacant(v) => {
                info!(
                    "direct add new device info: id={}, device={}",
                    name,
                    device.device.desc().device_id()
                );
                v.insert(device);

                Ok(())
            }
        }
    }

    // 获取desc，优先从缓存里面获取
    pub fn load(&mut self, name: &str) -> BuckyResult<DeviceInfo> {
        if let Some(desc) = self.list.get(name) {
            return Ok(desc.clone());
        }

        let desc = self.load_direct(name)?;
        let ret = desc.clone();
        self.list.insert(name.to_owned(), desc);

        Ok(ret)
    }

    pub fn load_direct(&self, name: &str) -> BuckyResult<DeviceInfo> {
        let desc_file_name = format!("{}.desc", name);
        let desc_file = self.root.join(desc_file_name);

        if !desc_file.is_file() {
            let msg = format!("desc file not exists! file={}", desc_file.display());
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        let mut buf: Vec<u8> = Vec::new();
        let ret = StandardObject::decode_from_file(desc_file.as_path(), &mut buf);
        if let Err(e) = ret {
            let msg = format!("load desc file error! file={}, {}", desc_file.display(), e,);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
        }

        info!("load desc success! file={}", desc_file.display());
        let (desc, _) = ret.unwrap();

        let device;
        match desc {
            StandardObject::Device(p) => {
                device = p;
            }
            other @ _ => {
                let msg = format!(
                    "unsupport desc type! file={}, desc={:?}",
                    desc_file.display(),
                    other
                );
                error!("{}", msg);

                return Err(BuckyError::new(BuckyErrorCode::UnSupport, msg));
            }
        };

        let mut device_info = DeviceInfo {
            device,
            private_key: None,
        };

        // 加载密钥
        let sec_file_name = format!("{}.sec", name);
        let sec_file = self.root.join(sec_file_name);
        if sec_file.is_file() {
            let mut buf: Vec<u8> = Vec::new();
            let ret = PrivateKey::decode_from_file(sec_file.as_path(), &mut buf);
            if let Err(e) = ret {
                let msg = format!("load sec file error! file={}, {}", sec_file.display(), e,);
                error!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::InvalidFormat, msg));
            }

            info!("load sec success! file={}", sec_file.display());

            let (private_key, _) = ret.unwrap();
            device_info.private_key = Some(private_key);
        } else {
            warn!("sec file not exists! file={}", sec_file.display());
        }

        Ok(device_info)
    }
}

#[derive(Clone)]
pub struct LocalDeviceManager(Arc<Mutex<LocalDeviceManagerImpl>>);

impl LocalDeviceManager {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(LocalDeviceManagerImpl::new())))
    }

    pub fn get_root(&self) -> PathBuf {
        self.0.lock().unwrap().get_root()
    }

    // 切换root
    pub fn set_root(&self, dir: &Path) {
        self.0.lock().unwrap().set_root(dir)
    }

    // 清空缓存
    pub fn clear_cache(&self) {
        self.0.lock().unwrap().clear_cache()
    }

    // 直接添加一个device
    pub fn add(&self, name: &str, device: DeviceInfo) -> BuckyResult<()> {
        self.0.lock().unwrap().add(name, device)
    }

    // 获取desc，优先从缓存里面获取
    pub fn load(&self, name: &str) -> BuckyResult<DeviceInfo> {
        self.0.lock().unwrap().load(name)
    }

    pub fn load_direct(&self, name: &str) -> BuckyResult<DeviceInfo> {
        self.0.lock().unwrap().load_direct(name)
    }
}

lazy_static! {
    pub static ref LOCAL_DEVICE_MANAGER: LocalDeviceManager = LocalDeviceManager::new();
}

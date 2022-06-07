use crate::package::ServicePackage;
use crate::config::ServiceConfig;
use cyfs_base::{BuckyError, BuckyErrorCode};
use cyfs_util::ZipPackage;

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};


// ood_daemon服务本身，需要特殊处理
pub const OOD_DAEMON_SERVICE: &str = "ood-daemon";

#[derive(Debug)]
pub(super) struct ServicePackageInfo {
    // service的根目录，一般是 /cyfs/services/[service_name]
    root: Option<PathBuf>,

    // 本身是不是就是ood_daemon服务
    as_ood_daemon: bool,

    // service的当前目录，和hash关联，一般是 /cyfs/services/[service_name]/[hash]
    current: Option<PathBuf>,

    // 在device_config里面指定
    name: String,
    version: String,

    // fid支持{dir_id}/{inner_path}模式，但本地目录只能使用第一段
    pub fid: String,
    pub full_fid: String,

    // 从service.cfg里面加载而来
    scripts: HashMap<String, String>,
}

impl ServicePackageInfo {
    pub fn new(service_config: &ServiceConfig) -> Self {

        let fid = service_config.fid.split("/").next().unwrap();
        
        Self {
            root: None,
            current: None,
            as_ood_daemon: service_config.name == OOD_DAEMON_SERVICE,
            name: service_config.name.clone(),
            fid: fid.to_owned(),
            full_fid: service_config.fid.clone(),
            version: service_config.version.clone(),
            scripts: HashMap::new(),
        }
    }

    pub fn root(&self) -> Option<PathBuf> {
        self.root.clone()
    }

    pub fn as_ood_daemon(&self) -> bool {
        self.as_ood_daemon
    }

    pub fn mark_ood_daemon(&mut self, as_ood_daemon: bool) {
        self.as_ood_daemon = as_ood_daemon;
    }

    pub fn current(&self) -> PathBuf {
        self.current.as_ref().unwrap().clone()
    }

    pub fn get_script(&self, name: &str) -> Option<String> {
        self.scripts.get(name).map(|v| v.to_owned())
    }

    pub fn bind(&mut self, root: &Path) {
        assert!(self.root.is_none());

        self.root = Some(root.to_path_buf());

        let current = root.join(self.fid.as_str());
        if !current.is_dir() {
            if let Err(e) = std::fs::create_dir_all(current.as_path()) {
                error!(
                    "create service current dir failed! dir={}, err={}",
                    current.display(),
                    e
                );
            } else {
                info!("create current dir: {} {}", self.name, current.display());
            }
        }

        info!("service current dir: {} {}", self.name, current.display());
        self.current = Some(current);

        if !self.scripts.is_empty() {
            self.bind_scripts();
        }
    }

    fn bind_scripts(&mut self) {
        let list: Vec<(String, String)> = self
            .scripts
            .iter()
            .map(|(k, v)| (k.to_string(), self.process_script(&v)))
            .collect();

        list.into_iter().for_each(|(k, v)| {
            self.scripts.insert(k, v);
        });
    }

    
    // 替换路径里面的预置变量，比如{root}
    fn process_script(&self, script: &String) -> String {
        assert_ne!(self.current, None);

        let root = self.current.as_ref().unwrap().to_str().unwrap();

        let ret = script.replace("{root}", root);
        debug!("service script bind {} => {}", script, ret);

        return ret;
    }

    // 加载package.cfg
    pub fn load_package(&mut self) -> Result<(), BuckyError> {
        assert!(self.current.is_some());

        // 首先判断目录是否存在
        if !self.current.as_ref().unwrap().is_dir() {
            let msg = format!(
                "service dir not found! service={}, dir={}",
                self.name,
                self.current.as_ref().unwrap().display(),
            );
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
        }

        // 尝试加载配置文件package.cfg
        let package_file = self.current.as_ref().unwrap().join("package.cfg");
        if !package_file.exists() {
            let msg = format!(
                "package.cfg not found! service={}, package_file={}",
                self.name,
                package_file.display()
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        let ret = File::open(package_file.as_path());
        if let Err(e) = ret {
            let msg = format!(
                "open package.cfg failed! service={}, package_file={}, err={}",
                self.name,
                package_file.display(),
                e
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        let reader = BufReader::new(ret.unwrap());
        let ret = serde_json::from_reader(reader);
        if let Err(e) = ret {
            let msg = format!(
                "invalid package.cfg format! service={}, package_file={}, err={}",
                self.name,
                package_file.display(),
                e
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        let u: Value = ret.unwrap();
        if !u.is_object() {
            let msg = format!(
                "invalid package.cfg top level format! service={}, package_file={}",
                self.name,
                package_file.display()
            );
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        return self.load_package_config(u.as_object().unwrap());
    }

    fn load_package_config(&mut self, service_node: &Map<String, Value>) -> Result<(), BuckyError> {
        for (k, v) in service_node {
            match k.as_str() {
                "version" => {
                    if v.is_string() {
                        self.version = v.as_str().unwrap_or("").to_string();
                    }
                }
                "scripts" => {
                    if let Some(scripts_node) = v.as_object() {
                        self.load_scripts(scripts_node)?;
                    } else {
                        let msg = format!("invalid scripts node format: {:?}", v);
                        error!("{}", msg);
                        return Result::Err(BuckyError::from(msg));
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn load_scripts(&mut self, scripts_node: &Map<String, Value>) -> Result<(), BuckyError> {
        // 需要清空老的scripts
        self.scripts.clear();

        for (k, v) in scripts_node {
            if let Some(value) = v.as_str() {
                info!("load service script item: {}: {}={}", self.name, k, value);

                let ret = self.scripts.insert(k.to_string(), value.to_string());
                if !ret.is_none() {
                    error!("repeat service script! service={}, name={}", self.name, k);
                }
            } else {
                error!("script value is not valid string! v={:?}", v);
            }
        }

        // 如果已经绑定了root，那么需要处理scripts
        if self.current.is_some() {
            self.bind_scripts();
        }

        Ok(())
    }

    pub fn load_package_file(&mut self, file: &Path) -> Result<bool, BuckyError> {

        // 解压到目标目录
        let pkg = ServicePackage::new(file).map_err(|e| {
            error!(
                "open service pkg error! file={}, err={}",
                file.display(),
                e
            );
            e
        })?;

        if let Err(e) = pkg.extract(self.current.as_ref().unwrap()) {
            error!(
                "extract service pkg error! file={}, err={}",
                file.display(),
                e
            );
            return Err(e);
        }

        // 提取成功后，删除临时文件
        if let Err(e) = std::fs::remove_file(file) {
            error!("remove service package temp file error! {}", e);
        }

        info!(
            "sync service package success! name={}, hash={}",
            self.name, self.fid
        );

        // 加载package.cfg
        self.load_package()?;

        // 加载成功后，更新current目录
        // FIXME 这个失败后暂时没影响
        ServicePackage::update_current(
            &self.root.as_ref().unwrap(),
            &self.current.as_ref().unwrap(),
            &self.fid,
        );

        Ok(true)
    }

    pub fn check_package(&self) -> bool {
        assert!(self.current.is_some());
        assert!(!self.fid.is_empty());

        if !self.current.as_ref().unwrap().exists() {
            warn!(
                "service current dir not exists! dir={}",
                self.current.as_ref().unwrap().display()
            );
            return false;
        }

        // 检查是否存在package.cfg
        let config_path = self.current.as_ref().unwrap().join("package.cfg");
        if !config_path.exists() {
            warn!(
                "package.cfg not found in service dir! file={}",
                config_path.display()
            );
            return false;
        }

        // ood_daemon不再检测本地包是否发生改变，只更新到最新的fid即可，避免和当前正在运行的程序冲突
        if self.as_ood_daemon() {
            return true
        }

        // 校验hash
        return match self.check_package_hash() {
            Ok(ret) => ret,
            Err(_e) => {
                // 计算出错如何处理？
                false
            }
        };
    }

    fn check_package_hash(&self) -> Result<bool, BuckyError> {
        // 检查是否存在.lock文件
        let lock_path = self.current.as_ref().unwrap().join(".lock");
        if lock_path.exists() {
            warn!("dir lock file exists! name={}, file={}", self.name, lock_path.display());
            return Ok(true);
        }

        // 检查是否存在.hash文件
        let hash_path = self.current.as_ref().unwrap().join(".hash");
        if !hash_path.exists() {
            let msg = format!(
                ".hash not found in service dir! file={}",
                hash_path.display()
            );
            error!("{}", msg);
            return Err(BuckyError::from(msg));
        }

        // 读取hash
        let configed_hash = match std::fs::read_to_string(hash_path.clone()) {
            Ok(hash) => hash,
            Err(e) => {
                let msg = format!("load .hash error! file={}, err={}", hash_path.display(), e);
                error!("{}", msg);
                return Err(BuckyError::from(msg));
            }
        };

        // 计算包的hash
        let mut zip = ZipPackage::new();
        zip.load(self.current.as_ref().unwrap());

        let dir_hash = match zip.calc_hash() {
            Ok(hash) => hash,
            Err(e) => {
                let msg = format!(
                    "calc dir hash error! dir={}, err={}",
                    self.current.as_ref().unwrap().display(),
                    e
                );
                error!("{}", msg);
                return Err(BuckyError::from(msg));
            }
        };
        if dir_hash != configed_hash {
            error!("dir hash not matched! now will reload service, service={}, dir={}, dir_hash={}, configed_hash={}",
             self.name, 
             self.current.as_ref().unwrap().display(),
             dir_hash, 
             configed_hash);
             
            return Ok(false);
        }

        Ok(true)
    }
}
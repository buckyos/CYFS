use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use cyfs_stack_loader::{DeviceInfo, LOCAL_DEVICE_MANAGER};
use cyfs_base::{
    BuckyError, BuckyResult, DeviceCategory, DeviceId, FileEncoder, NamedObject, ObjectId,
};
use desc_tool::desc;

pub struct DeviceDescGenerator {
    mac_address: String,

    pubkey_bits: usize, // RSA key bits
    owners: Option<ObjectId>,
    eps: Vec<String>,
    sn_list: Vec<DeviceId>,

    desc_file_path: PathBuf,
    sec_file_path: PathBuf,

    device_info: Option<DeviceInfo>,
    device_id: Option<DeviceId>,
}

impl DeviceDescGenerator {
    pub fn new() -> DeviceDescGenerator {
        let root = ::cyfs_util::get_cyfs_root_path().join("etc").join("desc");
        let desc_file_path = root.join("device.desc");
        let sec_file_path = root.join("device.sec");

        DeviceDescGenerator {
            mac_address: "".to_owned(),
            pubkey_bits: 1024, // rsa1024

            owners: None,
            eps: Vec::new(),
            sn_list: Vec::new(),

            desc_file_path,
            sec_file_path,

            device_info: None,
            device_id: None,
        }
    }

    pub fn get_device_id(&self) -> String {
        return self.device_id.as_ref().unwrap().to_string();
    }

    pub fn exists(&self) -> bool {
        return self.desc_file_path.exists() && self.sec_file_path.exists();
    }

    pub fn load(&mut self) -> BuckyResult<()> {
        let device_info = match LOCAL_DEVICE_MANAGER.load("device") {
            Ok(v) => v,
            Err(e) => {
                return Err(e);
            }
        };

        {
            let device_id = device_info.device.desc().device_id();

            info!("got device_id: {}", device_id);
            self.device_id = Some(device_id);
        }

        assert!(self.device_info.is_none());
        self.device_info = Some(device_info);

        Ok(())
    }

    pub async fn init(&mut self, overwrite: bool) -> BuckyResult<()> {
        // 保存文件路径
        let root = cyfs_util::get_cyfs_root_path().join("etc").join("desc");
        if let Err(e) = std::fs::create_dir_all(&root) {
            let msg = format!("create desc dir failed! dir={}, err={}", root.display(), e);
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        let desc_file_path = &self.desc_file_path;
        let sec_file_path = &self.sec_file_path;

        if desc_file_path.exists() || sec_file_path.exists() {
            if !overwrite {
                let msg = format!(
                    "device.desc or device.sec already exists! {}, {}\nto overwrite, use --force arg\nto ignore this phase, use --ignore\nand retry!!!",
                    desc_file_path.display(),
                    sec_file_path.display()
                );
                error!("{}", msg);
                return Err(BuckyError::from(msg));
            } else {
                let since_the_epoch = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis();
                let old_desc_file_path = root.join(format!("device_{}.desc", since_the_epoch));
                let old_sec_file_path = root.join(format!("device_{}.sec", since_the_epoch));

                warn!(
                    "device.desc or device.sec already exists, now will rename! {} => {} , {} => {}",
                    desc_file_path.display(),
                    old_desc_file_path.display(),
                    sec_file_path.display(),
                    old_sec_file_path.display()
                );

                if let Err(e) = fs::rename(&desc_file_path, &old_desc_file_path) {
                    error!(
                        "rename file error! {} => {}, err={}",
                        desc_file_path.display(),
                        old_desc_file_path.display(),
                        e
                    );
                    return Err(BuckyError::from(e));
                }
                if let Err(e) = fs::rename(&sec_file_path, &old_sec_file_path) {
                    error!(
                        "rename file error! {} => {}, err={}",
                        sec_file_path.display(),
                        old_sec_file_path.display(),
                        e
                    );
                    return Err(BuckyError::from(e));
                }
            }
        }

        self.local_init()
    }

    // 本地初始化，创建一个新的device.desc
    fn local_init(&mut self) -> BuckyResult<()> {
        info!("now will local init device.desc");
        // 初始化网卡地址
        if let Err(e) = self.init_mac() {
            return Err(e);
        }

        // 生成新的desc
        // OOD installer创建的Device一定是OOD类型
        let (device, secret) = match desc::create_device_desc(
            None,
            DeviceCategory::OOD,
            self.pubkey_bits,
            &self.mac_address,
            self.owners.clone(),
            self.eps.clone(),
            self.sn_list.clone(),
            None,
        ) {
            Some(device) => device,
            None => {
                let msg = format!("create device desc error!");
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        {
            let device_id = device.desc().device_id();

            info!("gen new peerid: {}", device_id.to_string());
            self.device_id = Some(device_id);
        }

        assert!(self.device_info.is_none());
        self.device_info = Some(DeviceInfo {
            device,
            private_key: Some(secret),
        });

        // 保存
        if let Err(e) = self.save() {
            return Err(e);
        }

        Ok(())
    }

    fn init_mac(&mut self) -> BuckyResult<()> {
        let address = match mac_address::get_mac_address() {
            Ok(ret) => {
                if ret.is_some() {
                    ret.unwrap()
                } else {
                    let msg = format!("get mac is none!");
                    error!("{}", msg);

                    return Err(BuckyError::from(msg));
                }
            }
            Err(e) => {
                let msg = format!("get mac address error! err={}", e);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        self.mac_address = hex::encode(address.bytes());

        info!("retrive mac address: {}", self.mac_address);

        Ok(())
    }

    fn save(&self) -> BuckyResult<()> {
        let device_info = self.device_info.as_ref().unwrap();

        match device_info
            .private_key
            .as_ref()
            .unwrap()
            .encode_to_file(&self.sec_file_path, true)
        {
            Ok(n) => {
                info!(
                    "encode sceret to file success: {}, count={}",
                    self.sec_file_path.display(),
                    n
                );
            }
            Err(e) => {
                let msg = format!(
                    "encode sceret to file failed, file={}, e={}",
                    self.sec_file_path.display(),
                    e
                );

                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        match device_info
            .device
            .encode_to_file(&self.desc_file_path, true)
        {
            Ok(n) => {
                info!(
                    "encode desc to file success: {}, count={}",
                    self.desc_file_path.display(),
                    n
                );
            }
            Err(e) => {
                let msg = format!(
                    "encode desc to file failed, file={}, e={}",
                    self.desc_file_path.display(),
                    e
                );

                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        Ok(())
    }
}

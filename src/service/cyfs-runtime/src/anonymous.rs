use cyfs_stack_loader::LOCAL_DEVICE_MANAGER;
use cyfs_base::*;
use cyfs_stack_loader::DeviceInfo;

use rand::Rng;
use std::path::PathBuf;

// Anonymous id manager
pub(crate) struct AnonymousManager {
    dir: PathBuf,
    desc_file: PathBuf,
    sec_file: PathBuf,
}

impl AnonymousManager {
    pub fn new() -> Self {
        let dir = cyfs_util::get_cyfs_root_path()
            .join("etc")
            .join("desc")
            .join("anonymous");
        if !dir.exists() {
            if let Err(e) = std::fs::create_dir_all(dir.as_path()) {
                error!("create anonymous dir error! dir={}, {}", dir.display(), e);
            }
        }

        let ret = AnonymousManager {
            desc_file: dir.join("device.desc"),
            sec_file: dir.join("device.sec"),
            dir,
        };

        ret
    }

    pub fn init(&self, random_id: bool) -> String {
        let info = self.init_device(random_id);

        // add to local_device_manager, so cyfs-stack-loader will load device from it by the device_id string
        let device_id = info.device.desc().calculate_id();
        let id = device_id.to_string();
        if let Err(e) = LOCAL_DEVICE_MANAGER.add(&id, info) {
            error!(
                "add anonymous device to local_device_manager error! id={}, {}",
                id, e
            );
        }

        id
    }

    fn init_device(&self, random_id: bool) -> DeviceInfo {
        if !random_id {
            if let Ok(Some(info)) = self.load_device() {
                return info;
            }
        }

        let (id, info) = self.gen_random_device();
        if let Err(e) = self.save_device(&info) {
            error!("save anonymous device error! id={}, {}", id, e);
        }

        info
    }

    fn load_device(&self) -> BuckyResult<Option<DeviceInfo>> {
        if self.desc_file.is_file() && self.sec_file.is_file() {
            let (private_key, _) =
                PrivateKey::decode_from_file(self.sec_file.as_path(), &mut Vec::new())?;

            let (device, _) = Device::decode_from_file(self.desc_file.as_path(), &mut Vec::new())?;

            info!(
                "load anonymous device success! desc={}, id={}",
                self.desc_file.display(),
                device.desc().calculate_id()
            );
            let info = DeviceInfo {
                device,
                private_key: Some(private_key),
            };
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    fn save_device(&self, info: &DeviceInfo) -> BuckyResult<()> {
        self.rename_current();

        // 保存到文件
        info.device
            .encode_to_file(&self.desc_file, false)
            .map_err(|e| {
                let msg = format!(
                    "save device.desc to file error! file={}, {}",
                    self.desc_file.display(),
                    e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        info.private_key
            .as_ref()
            .unwrap()
            .encode_to_file(&self.sec_file, false)
            .map_err(|e| {
                let msg = format!(
                    "save device.sec to file error! file={}, {}",
                    self.sec_file.display(),
                    e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        Ok(())
    }

    fn rename_current(&self) {
        let since_the_epoch = bucky_time_now();

        if self.desc_file.exists() {
            let old_desc_file_path = format!("{}.{}", self.desc_file.display(), since_the_epoch);

            warn!(
                "device.desc already exists, now will rename! {} => {}",
                self.desc_file.display(),
                old_desc_file_path,
            );

            if let Err(e) = std::fs::rename(&self.desc_file, &old_desc_file_path) {
                error!(
                    "rename desc file error! {} => {}, err={}",
                    self.desc_file.display(),
                    old_desc_file_path,
                    e
                );
            }
        }

        if self.sec_file.exists() {
            let old_sec_file_path = format!("{}.{}", self.sec_file.display(), since_the_epoch);

            warn!(
                "device.sec already exists, now will rename! {} => {}",
                self.sec_file.display(),
                old_sec_file_path,
            );

            if let Err(e) = std::fs::rename(&self.sec_file, &old_sec_file_path) {
                error!(
                    "rename sec file error! {} => {}, err={}",
                    self.sec_file.display(),
                    old_sec_file_path,
                    e
                );
            }
        }
    }

    pub fn gen_random_device(&self) -> (DeviceId, DeviceInfo) {
        let area = Area::new(0, 0, 0, 0);

        let private_key = PrivateKey::generate_rsa(1024).unwrap();
        let public_key = private_key.public();

        let index = rand::thread_rng().gen::<u64>();
        let id = format!("anon-dev-{}", index);
        let uni_id = UniqueId::create(id.as_bytes());

        let device = Device::new(
            None,
            uni_id,
            vec![],
            vec![],
            vec![],
            public_key,
            area,
            DeviceCategory::OOD,
        )
        .build();

        let device_id = device.desc().device_id();
        info!("gen new anonymous device success! id={}", device_id);

        let info = DeviceInfo {
            device,
            private_key: Some(private_key),
        };

        (device_id, info)
    }
}

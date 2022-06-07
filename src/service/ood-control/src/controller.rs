use super::bind::*;
use super::device_info::*;
use super::request::*;
use cyfs_base::*;

use once_cell::sync::OnceCell;
use std::collections::hash_map::{Entry, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub(crate) struct ControllerImpl {
    desc_file: PathBuf,
    sec_file: PathBuf,
    ext_info_file: PathBuf,
    zone_owner_desc_file: PathBuf,

    // 设备信息
    device_info: DeviceInfo,

    bind_state: BindState,

    check_status: Mutex<HashMap<String, CheckStatus>>,

    access_info: OnceCell<ControlInterfaceAccessInfo>,
}

impl ControllerImpl {
    pub fn new() -> Self {
        let (desc_file, sec_file, ext_info_file, zone_owner_desc_file) =
            Self::get_device_file_path();

        let bind_state = BindState::new(desc_file.clone(), sec_file.clone(), ext_info_file.clone());
        let _r = bind_state.load();

        // 如果尚未绑定，那么开启一个检测
        if !bind_state.is_bind() {
            bind_state.start_monitor_bind();
        }

        let check_status = HashMap::new();

        Self {
            desc_file,
            sec_file,
            ext_info_file,
            zone_owner_desc_file,

            bind_state,
            device_info: DeviceInfoGen::new(),

            check_status: Mutex::new(check_status),

            access_info: OnceCell::new(),
        }
    }

    pub fn init_access_info(&self, access_info: ControlInterfaceAccessInfo) {
        if let Err(_) = self.access_info.set(access_info) {
            unreachable!();
        }
    }

    pub fn is_bind(&self) -> bool {
        self.bind_state.is_bind()
    }

    pub fn on_check_request(&self, source: &str) {
        let mut check_status = self.check_status.lock().unwrap();
        match check_status.entry(source.to_owned()) {
            Entry::Vacant(v) => {
                let status = CheckStatus {
                    access_count: 1,
                    last_access: bucky_time_now(),
                };
                v.insert(status);
            }
            Entry::Occupied(mut o) => {
                o.get_mut().access_count += 1;
                o.get_mut().last_access = bucky_time_now();
            }
        }
    }

    pub async fn check(&self) -> CheckResponse {
        CheckResponse {
            activation: self.bind_state.is_bind(),
            check_status: self.check_status.lock().unwrap().clone(),
            device_info: self.device_info.clone(),
            access_info: self.access_info.get().unwrap().clone(),
            bind_info: Some(self.bind_state.fill_bind_info()),
        }
    }

    pub async fn bind(&self, info: ActivateInfo) -> ActivateResult {
        match self.bind_impl(info).await {
            Ok(_) => ActivateResult {
                result: 0,
                msg: "".to_owned(),
            },
            Err(e) => ActivateResult {
                result: e.code().into(),
                msg: e.msg().to_owned(),
            },
        }
    }

    fn verify_zone(owner: &People, device: &Device) -> BuckyResult<()> {
        let owner_id = owner.desc().calculate_id();

        // 检查people的ood_list是否为空
        if owner.ood_list().is_empty() {
            let msg = format!("device's owner's ood_list is empty! owner={}", owner_id);
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::ErrorState, msg));
        }

        // 检查device的owner是否正确
        if device.desc().owner() != &Some(owner_id) {
            let msg = format!(
                "device's owner is unmatch! owner={}, device's owner={:?}",
                owner_id,
                device.desc().owner()
            );
            error!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        Ok(())
    }

    async fn bind_impl(&self, info: ActivateInfo) -> BuckyResult<()> {
        if self.is_bind() {
            let msg = format!("device already been binded!");
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
        }

        let owner = People::clone_from_hex(info.owner.as_str(), &mut Vec::new()).map_err(|e| {
            let msg = format!("decode people error! buf={}, {}", info.owner, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let device = Device::clone_from_hex(info.desc.as_str(), &mut Vec::new()).map_err(|e| {
            let msg = format!("decode device error! buf={}, {}", info.owner, e);
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        // 确保owner已经绑定了ood
        Self::verify_zone(&owner, &device)?;

        let private_key =
            PrivateKey::clone_from_hex(info.sec.as_str(), &mut Vec::new()).map_err(|e| {
                let msg = format!("decode private_key error! {}", e);
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
            })?;

        // 校验owner签名是否有效
        let verifier_ret = Self::verify_sign(&owner, &device).await;
        if !verifier_ret {
            let msg = format!("verify device sign by owner failed!");
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::InvalidSignature, msg));
        }

        // 尝试重命名已有的
        self.rename_current();

        // 保存到文件
        device.encode_to_file(&self.desc_file, false).map_err(|e| {
            let msg = format!(
                "save device.desc to file error! file={}, {}",
                self.desc_file.display(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        private_key
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

        // try save zone owner desc
        let _ = owner
            .encode_to_file(&self.zone_owner_desc_file, false)
            .map_err(|e| {
                let msg = format!(
                    "save zone_owner.desc to file error! file={}, {}",
                    self.desc_file.display(),
                    e
                );
                error!("{}", msg);

                BuckyError::new(BuckyErrorCode::IoError, msg)
            });

        let device_id = device.desc().device_id().to_string();
        info!("bind device success! device={}", device_id);

        self.bind_state
            .on_bind(info.owner.clone(), info.desc.clone(), device_id, info.index);
        Ok(())
    }

    async fn verify_sign(owner: &People, device: &Device) -> bool {
        let verifier = RsaCPUObjectVerifier::new(owner.desc().public_key().clone());

        let desc_signs = device.signs().desc_signs();
        if desc_signs.is_none() {
            error!("device desc had now sign!");
            return false;
        }

        let signs = desc_signs.as_ref().unwrap();
        if signs.len() == 0 {
            error!("device desc signs list is empty!");
            return false;
        }

        // 校验签名
        for sign in signs.iter() {
            match cyfs_base::verify_object_desc_sign(&verifier, device, sign).await {
                Ok(result) => {
                    if result {
                        return true;
                    }
                }
                Err(e) => {
                    error!("verify device sign error! sign={:?}, {}", sign, e);
                }
            }
        }

        error!(
            "device sign verify by people failed! device={:?}, people={:?}",
            device, owner
        );
        false
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

        if self.ext_info_file.exists() {
            let old_ext_info_file_path =
                format!("{}.{}", self.ext_info_file.display(), since_the_epoch);

            warn!(
                "ext_info already exists, now will rename! {} => {}",
                self.ext_info_file.display(),
                old_ext_info_file_path,
            );

            if let Err(e) = std::fs::rename(&self.sec_file, &old_ext_info_file_path) {
                error!(
                    "rename ext_info file error! {} => {}, err={}",
                    self.ext_info_file.display(),
                    old_ext_info_file_path,
                    e
                );
            }
        }
    }

    fn get_device_file_path() -> (PathBuf, PathBuf, PathBuf, PathBuf) {
        let device_path = cyfs_util::get_cyfs_root_path().join("etc").join("desc");
        if !device_path.exists() {
            if let Err(e) = std::fs::create_dir_all(device_path.as_path()) {
                error!("create dir error! dir={}, {}", device_path.display(), e);
            }
        }

        (
            device_path.join("device.desc"),
            device_path.join("device.sec"),
            device_path.join("ext_info"),
            device_path.join("zone_owner.desc"),
        )
    }
}

#[derive(Clone)]
pub struct Controller(Arc<ControllerImpl>);

impl Controller {
    pub(crate) fn new() -> Self {
        Self(Arc::new(ControllerImpl::new()))
    }

    // 记录一次check请求
    pub fn on_check_request(&self, source: &str) {
        self.0.on_check_request(source)
    }

    pub async fn check(&self) -> CheckResponse {
        self.0.check().await
    }

    pub async fn bind(&self, info: ActivateInfo) -> ActivateResult {
        self.0.bind(info).await
    }

    pub fn is_bind(&self) -> bool {
        self.0.is_bind()
    }

    pub fn bind_event(&self) -> OnBindEventManager {
        self.0.bind_state.bind_event()
    }

    pub fn fill_bind_info(&self) -> BindInfo {
        self.0.bind_state.fill_bind_info()
    }

    pub fn init_access_info(&self, access_info: ControlInterfaceAccessInfo) {
        self.0.init_access_info(access_info)
    }
}

use lazy_static::lazy_static;

lazy_static! {
    pub static ref OOD_CONTROLLER: Controller = Controller::new();
}

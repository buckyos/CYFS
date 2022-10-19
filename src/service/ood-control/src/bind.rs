use super::request::*;
use cyfs_base::*;
use cyfs_util::*;

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use cyfs_debug::Mutex;

// 保存的扩展信息
#[derive(Serialize, Deserialize)]
pub(super) struct BindExtInfo {
    // 绑定的people
    owner: String,

    // 绑定设备对应people的索引
    index: i32,
}

impl BindExtInfo {
    pub fn load(file: &Path) -> BuckyResult<Self> {
        match std::fs::read_to_string(file) {
            Ok(v) => {
                let ret = serde_json::from_str(&v).map_err(|e| {
                    let msg = format!(
                        "parse bind ext info file error! file={}, value={}, {}",
                        file.display(),
                        v,
                        e
                    );
                    error!("{}", msg);

                    BuckyError::from(msg)
                })?;

                Ok(ret)
            }
            Err(e) => {
                let msg = format!(
                    "load bind ext info file error! file={}, {}",
                    file.display(),
                    e
                );
                warn!("{}", msg);

                Err(BuckyError::from(msg))
            }
        }
    }

    pub fn save(&self, file: &Path) -> BuckyResult<()> {
        let value = serde_json::to_string(self).map_err(|e| {
            let msg = format!("encode bind ext info to string failed! {}", e);
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        std::fs::write(file, &value).map_err(|e| {
            let msg = format!(
                "save bind ext info to file failed! file={}, {}",
                file.display(),
                e
            );
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        info!(
            "save bind ext info to file success! file={}, info={}",
            file.display(),
            value
        );
        Ok(())
    }
}

pub(super) struct BindStateImpl {
    desc_file: PathBuf,
    sec_file: PathBuf,
    ext_info_file: PathBuf,

    is_bind: bool,

    // 绑定的设备描述相关
    device: Option<String>,
    device_id: Option<String>,

    bind_ext_info: Option<BindExtInfo>,

    // 事件
    on_bind: OnBindEventManager,
}

impl BindStateImpl {
    pub fn new(desc_file: PathBuf, sec_file: PathBuf, ext_info_file: PathBuf) -> Self {
        Self {
            desc_file,
            sec_file,
            ext_info_file,
            is_bind: false,
            device: None,
            device_id: None,
            bind_ext_info: None,
            on_bind: OnBindEventManager::new(),
        }
    }

    fn emit_bind_event(&self) {
        // 触发事件
        let event = self.on_bind.clone();
        async_std::task::spawn(async move {
            let _ = event.emit(&());
        });
    }

    pub fn on_bind(&mut self, owner: String, device: String, device_id: String, index: i32) {
        let bind_ext_info = BindExtInfo { owner, index };
        let _ = bind_ext_info.save(&self.ext_info_file);

        self.bind_ext_info = Some(bind_ext_info);
        self.device = Some(device);
        self.device_id = Some(device_id);

        self.is_bind = true;

        self.emit_bind_event();
    }

    pub fn is_bind(&self) -> bool {
        self.is_bind
    }

    pub fn is_desc_exists(&self) -> bool {
        self.desc_file.exists() && self.sec_file.exists()
    }

    pub fn load(&mut self) -> BuckyResult<()> {
        if !self.is_desc_exists() {
            warn!(
                "desc_file or sec_file not exists! {}, {}",
                self.desc_file.display(),
                self.sec_file.display()
            );
            self.is_bind = false;
            return Ok(());
        }

        let mut buf = Vec::new();
        let (device, len) = Device::decode_from_file(&self.desc_file, &mut buf).map_err(|e| {
            let msg = format!(
                "load device file error! file={}, {}",
                self.desc_file.display(),
                e
            );
            error!("{}", msg);

            BuckyError::from(msg)
        })?;

        self.device = Some(::hex::encode(&buf[..len]));
        self.device_id = Some(device.desc().device_id().to_string());

        if let Ok(v) = BindExtInfo::load(&self.ext_info_file) {
            self.bind_ext_info = Some(v);
        }

        self.is_bind = true;
        self.emit_bind_event();

        Ok(())
    }

    pub fn fill_bind_info(&self) -> BindInfo {
        let mut bind_info = BindInfo {
            unique_id: "".to_owned(),
            area: "".to_owned(),
            owner_id: "".to_owned(),
            index: -1,
            device_id: "".to_owned(),
            device: "".to_owned(),
            name: "".to_owned(),
        };

        if let Some(ref device) = self.device {
            bind_info.device = device.to_owned();

            match Device::clone_from_hex(&device, &mut Vec::new()) {
                Ok(d) => {
                    bind_info.device_id = d.desc().device_id().to_string();
                    match d.desc().owner() {
                        Some(owner) => {
                            bind_info.owner_id = owner.to_string();
                        }
                        None => {
                            error!("device has no owner! device={}", device);
                        }
                    };

                    match d.name() {
                        Some(name) => {
                            bind_info.name = name.to_owned();
                        }
                        None => {
                            warn!("device has no name! device={}", device);
                        }
                    }
                    match d.desc().area() {
                        Some(area) => {
                            bind_info.area = area.to_string();
                        }
                        None => {
                            error!("device has no area! device={}", device);
                        }
                    }

                    bind_info.unique_id = d.desc().unique_id().to_string();
                }
                Err(e) => {
                    error!("decode device.desc string failed! device={}, {}", device, e);
                }
            }
        }

        if let Some(ref ext_info) = self.bind_ext_info {
            bind_info.index = ext_info.index;
        }

        bind_info
    }
}

pub type FnOnBind = dyn EventListenerAsyncRoutine<(), ()>;
pub type OnBindEventManager = SyncEventManagerSync<(), ()>;

#[derive(Clone)]
pub(super) struct BindState {
    state: Arc<Mutex<BindStateImpl>>,
}

impl BindState {
    pub fn new(desc_file: PathBuf, sec_file: PathBuf, ext_info_file: PathBuf) -> Self {
        let state = Arc::new(Mutex::new(BindStateImpl::new(
            desc_file,
            sec_file,
            ext_info_file,
        )));
        Self { state }
    }

    pub fn start_monitor_bind(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            loop {
                if this.is_bind() {
                    info!("monitor bind success! now will stop monitor!");
                    break;
                }

                if this.is_desc_exists() {
                    let _r = this.load();

                    if this.is_bind() {
                        info!("monitor bind success! now will stop monitor!");
                        break;
                    }
                }

                async_std::task::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
    }

    fn is_desc_exists(&self) -> bool {
        self.state.lock().unwrap().is_desc_exists()
    }

    pub fn on_bind(&self, owner: String, device: String, device_id: String, index: i32) {
        self.state
            .lock()
            .unwrap()
            .on_bind(owner, device, device_id, index)
    }

    pub fn is_bind(&self) -> bool {
        self.state.lock().unwrap().is_bind()
    }

    pub fn load(&self) -> BuckyResult<()> {
        self.state.lock().unwrap().load()
    }

    pub fn fill_bind_info(&self) -> BindInfo {
        self.state.lock().unwrap().fill_bind_info()
    }

    pub fn bind_event(&self) -> OnBindEventManager {
        self.state.lock().unwrap().on_bind.clone()
    }
}

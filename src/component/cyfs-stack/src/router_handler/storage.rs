use cyfs_base::*;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct RouterHandlerSavedData {
    pub index: i32,

    pub filter: String,

    pub default_action: String,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RouterHandlerContainerSavedData {
    pub put_object: Option<BTreeMap<String, RouterHandlerSavedData>>,
    pub get_object: Option<BTreeMap<String, RouterHandlerSavedData>>,
    pub post_object: Option<BTreeMap<String, RouterHandlerSavedData>>,
    pub select_object: Option<BTreeMap<String, RouterHandlerSavedData>>,
    pub delete_object: Option<BTreeMap<String, RouterHandlerSavedData>>,

    pub get_data: Option<BTreeMap<String, RouterHandlerSavedData>>,
    pub put_data: Option<BTreeMap<String, RouterHandlerSavedData>>,
    pub delete_data: Option<BTreeMap<String, RouterHandlerSavedData>>,

    pub sign_object: Option<BTreeMap<String, RouterHandlerSavedData>>,
    pub verify_object: Option<BTreeMap<String, RouterHandlerSavedData>>,

    pub acl: Option<BTreeMap<String, RouterHandlerSavedData>>,
}

impl RouterHandlerContainerSavedData {
    pub fn new() -> Self {
        Self {
            put_object: None,
            get_object: None,
            post_object: None,
            select_object: None,
            delete_object: None,

            get_data: None,
            put_data: None,
            delete_data: None,

            sign_object: None,
            verify_object: None,

            acl: None,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.put_object.is_none()
            && self.get_object.is_none()
            && self.post_object.is_none()
            && self.select_object.is_none()
            && self.delete_object.is_none()
            && self.get_data.is_none()
            && self.put_data.is_none()
            && self.delete_data.is_none()
            && self.sign_object.is_none()
            && self.verify_object.is_none()
            && self.acl.is_none()
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct RouterHandlersSavedData {
    pub pre_noc: Option<RouterHandlerContainerSavedData>,
    pub post_noc: Option<RouterHandlerContainerSavedData>,

    pub pre_router: Option<RouterHandlerContainerSavedData>,
    pub post_router: Option<RouterHandlerContainerSavedData>,

    pub pre_forward: Option<RouterHandlerContainerSavedData>,
    pub post_forward: Option<RouterHandlerContainerSavedData>,

    pub pre_crypto: Option<RouterHandlerContainerSavedData>,
    pub post_crypto: Option<RouterHandlerContainerSavedData>,

    pub handler: Option<RouterHandlerContainerSavedData>,
    
    pub acl: Option<RouterHandlerContainerSavedData>,
}

impl RouterHandlersSavedData {
    pub fn new() -> Self {
        Self {
            pre_noc: None,
            post_noc: None,
            pre_router: None,
            post_router: None,
            pre_forward: None,
            post_forward: None,
            
            pre_crypto: None,
            post_crypto: None,

            handler: None,

            acl: None,
        }
    }
}

// declare_collection_codec_for_serde!(RouterHandlersSavedData);

use super::handler_manager::RouterHandlersManager;
use std::path::PathBuf;
use std::sync::Arc;

struct RouterHandlersStorageImpl {
    file: PathBuf,
    // storage: NOCStorageWrapper,
    handler_manager: once_cell::sync::OnceCell<RouterHandlersManager>,
}

impl RouterHandlersStorageImpl {
    pub fn new(config_isolate: Option<String>) -> Self {
        let mut file = cyfs_util::get_cyfs_root_path().join("etc");
        if let Some(isolate) = &config_isolate {
            if isolate.len() > 0 {
                file.push(isolate.as_str());
            }
        }

        file.push("handler");
        file.push("handler.toml");

        Self {
            file,
            handler_manager: once_cell::sync::OnceCell::new(),
        }
    }

    pub async fn load(&self) -> BuckyResult<()> {
        if !self.file.exists() {
            return Ok(());
        }

        let value = async_std::fs::read_to_string(&self.file)
            .await
            .map_err(|e| {
                let msg = format!(
                    "load handlers from config error! file={}, {}",
                    self.file.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

        info!(
            "will load handler config: file={}, {}",
            self.file.display(),
            value
        );

        let data: RouterHandlersSavedData = toml::from_str(&value).map_err(|e| {
            let msg = format!(
                "invalid handlers config! file={}, {}",
                self.file.display(),
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        self.handler_manager.get().unwrap().load_data(data);
        Ok(())

        /*
        match self.storage.load::<RouterHandlersSavedData>().await {
            Ok(Some(data)) => {
                self.handler_manager.get().unwrap().load_data(data);
                Ok(())
            }
            Ok(None) => Ok(()),
            Err(e) => {
                error!("load router handler saved data from noc error: {}", e);
                Err(e)
            }
        }
        */
    }

    pub async fn save(&self) -> BuckyResult<()> {
        let data = self.handler_manager.get().unwrap().dump_data();

        if !self.file.exists() {
            let dir = self.file.parent().unwrap();
            if !dir.is_dir() {
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    error!(
                        "create handler config dir error! dir={}, {}",
                        dir.display(),
                        e
                    );
                }
            }
        }

        let data = toml::to_string(&data).unwrap();
        async_std::fs::write(&self.file, &data).await.map_err(|e| {
            let msg = format!(
                "write handler to config file error! file={}, {}, {}",
                self.file.display(),
                data,
                e
            );
            error!("{}", msg);
            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        info!(
            "save router handlers to config file success! file={}, {}",
            self.file.display(),
            data
        );

        Ok(())
    }
}

#[derive(Clone)]
pub struct RouterHandlersStorage {
    save_lock: Arc<async_std::sync::Mutex<bool>>,
    storage: Arc<RouterHandlersStorageImpl>,
}

impl RouterHandlersStorage {
    pub fn new(config_isolate: Option<String>) -> Self {
        Self {
            save_lock: Arc::new(async_std::sync::Mutex::new(true)),
            storage: Arc::new(RouterHandlersStorageImpl::new(config_isolate)),
        }
    }

    pub fn bind(&self, handler_manager: RouterHandlersManager) {
        if let Err(_) = self.storage.handler_manager.set(handler_manager) {
            unreachable!();
        }
    }

    pub async fn load(&self) -> BuckyResult<()> {
        self.storage.load().await
    }

    pub fn async_save(&self) {
        let this = self.clone();
        async_std::task::spawn(async move {
            // 确保save操作不并发
            let _holder = this.save_lock.lock().await;

            // TODO 失败后继续重试
            let _ = this.storage.save().await;
        });
    }
}

use cyfs_base::BuckyError;
use crate::get_cyfs_root_path;

use async_std::fs;
use async_trait::async_trait;
use std::collections::BTreeMap;
use std::path::PathBuf;

pub struct FileStorage {
    loaded: bool,
    path: PathBuf,
    dirty: bool,
    storage: BTreeMap<String, String>,
}

impl FileStorage {
    pub fn new() -> FileStorage {
        FileStorage {
            loaded: false,
            path: PathBuf::from(""),
            dirty: false,
            storage: BTreeMap::new(),
        }
    }

    pub async fn init(&mut self, service_name: &str) -> Result<(), BuckyError> {
        assert!(!self.loaded);
        self.loaded = true;

        let dir = get_cyfs_root_path().join("profile").join(service_name);
        if !dir.is_dir() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let msg = format!("create profile dir error! dir={}, err={}", dir.display(), e);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        }

        let file = dir.join("profile.json");
        self.path = file;
        if !self.path.exists() {
            info!("file storage file not exists! file={}", self.path.display());
            return Ok(());
        }

        return self.load().await;
    }

    async fn load(&mut self) -> Result<(), BuckyError> {
        assert!(self.storage.is_empty());

        let contents = match fs::read_to_string(&self.path).await {
            Ok(v) => v,
            Err(e) => {
                let msg = format!(
                    "load storage file as string error! file={}, err={}",
                    self.path.display(),
                    e
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        self.storage = match serde_json::from_str(&contents) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!(
                    "unserialize storage file from string error! file={}, err={}, content={}",
                    self.path.display(),
                    e,
                    contents
                );
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        self.dirty = false;
        info!("load storage success! file={}", self.path.display());

        Ok(())
    }

    pub async fn get_item_str(&self, key: &str) -> Option<&str> {
        self.storage
            .get_key_value(key)
            .map(|(_, value)| value.as_str())
    }

    async fn flush(&mut self) -> Result<(), BuckyError> {
        if !self.dirty {
            return Ok(());
        }

        let str_value = match serde_json::to_string(&self.storage) {
            Ok(v) => v,
            Err(e) => {
                let msg = format!("storage to string error! err={}", e);
                error!("{}", msg);

                return Err(BuckyError::from(msg));
            }
        };

        if let Err(e) = fs::write(&self.path, str_value.as_bytes()).await {
            let msg = format!("write file error! file={}, err={}", self.path.display(), e);
            error!("{}", msg);

            return Err(BuckyError::from(msg));
        }

        assert!(self.dirty);
        self.dirty = false;

        Ok(())
    }
}

#[async_trait]
impl super::AsyncStorage for FileStorage {
    async fn set_item(&mut self, key: &str, value: String) -> Result<(), BuckyError> {
        match self.storage.insert(key.to_owned(), value) {
            Some(v) => {
                info!("key replace: {}, old: {}", key, v);
            }
            None => {
                info!("key insert: {}", key);
            }
        };

        self.dirty = true;

        self.flush().await
    }

    async fn get_item(&self, key: &str) -> Option<String> {
        self.storage
            .get_key_value(key)
            .map(|(_, value)| value.clone())
    }

    async fn remove_item(&mut self, key: &str) -> Option<()> {
        match self.storage.remove(key) {
            Some(value) => {
                self.dirty = true;
                let _r = self.flush().await;

                info!("key removed: key={}, value={}", key, value);

                //Some(value)
                Some(())
            }
            None => None,
        }
    }

    async fn clear(&mut self) {
        if !self.storage.is_empty() {
            self.storage.clear();

            self.dirty = true;
            let _r = self.flush().await;

            info!("storage cleared: file={}", self.path.display());
        }
    }

    async fn clear_with_prefix(&mut self, prefix: &str) {
        let keys: Vec<String> = self
            .storage
            .keys()
            .filter_map(|key| {
                if key.starts_with(prefix) {
                    Some(key.to_owned())
                } else {
                    None
                }
            })
            .collect();

        for key in keys {
            match self.storage.remove(&key) {
                Some(value) => {
                    self.dirty = true;

                    info!("key removed: key={}, value={}", key, value);
                }
                None => {
                    error!("key not found： key={}", key);
                }
            }
        }

        if self.dirty {
            let _r = self.flush().await;
        }
    }
}

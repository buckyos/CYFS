use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_lib::NONObjectInfo;
use cyfs_util::DirObjectsSyncLoader;
use cyfs_stack::CyfsStackKnownObjectsInitMode;

use lazy_static::lazy_static;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct KnownObjectsLoader {
    desc_folder: PathBuf,
    objects: Vec<NONObjectInfo>,
}

impl KnownObjectsLoader {
    pub fn new() -> Self {
        let desc_folder = cyfs_util::get_cyfs_root_path().join("etc").join("desc");

        Self {
            desc_folder,
            objects: Vec::new(),
        }
    }

    pub async fn load(&mut self) {
        let mut loader = DirObjectsSyncLoader::new(self.desc_folder.clone());
        loader.load();

        let objects = loader.into_objects();
        for (file_path, data) in objects {
            let ret = self.load_obj(&file_path, data).await;
            if ret.is_err() {
                continue;
            }

            let ret = ret.unwrap();

            if !self
                    .objects
                    .iter()
                    .any(|item| item.object_id == ret.object_id)
                {
                    self.objects.push(ret);
                } else {
                    warn!(
                        "object already in list! id={}, file={}",
                        ret.object_id,
                        file_path.display()
                    );
                }
        }
    }
   
    async fn load_obj(&self, file: &Path, buf: Vec<u8>) -> BuckyResult<NONObjectInfo> {
        let object = NONObjectInfo::new_from_object_raw(buf).map_err(|e| {
            let msg = format!(
                "invalid known object buffer: file={}, {}",
                file.display(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;
    
        Ok(object)
    }
}

pub(crate) struct KnownObjectsManagerImpl {
    objects: Vec<NONObjectInfo>,
    mode: CyfsStackKnownObjectsInitMode,
}

impl KnownObjectsManagerImpl {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            mode: CyfsStackKnownObjectsInitMode::Async,
        }
    }

    pub async fn load(&mut self) {
        let mut loader = KnownObjectsLoader::new();
        loader.load().await;

        self.append(loader.objects);
    }

    pub fn append(&mut self, known_objects: Vec<NONObjectInfo>) {
        for item in known_objects.into_iter() {
            if !self.objects.iter().any(|v| v.object_id == item.object_id) {
                self.objects.push(item);
            } else {
                warn!("object already in list! id={}", item.object_id);
            }
        }
    }

    pub fn clear(&mut self) {
        warn!("will clear all known objects!");
        self.objects.clear();
    }
}

pub struct KnownObjectsManager(Arc<Mutex<KnownObjectsManagerImpl>>);

impl KnownObjectsManager {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(KnownObjectsManagerImpl::new())))
    }

    pub fn get_mode(&self) -> CyfsStackKnownObjectsInitMode {
        self.0.lock().unwrap().mode.clone()
    }

    pub fn set_mode(&self, mode: CyfsStackKnownObjectsInitMode) {
        self.0.lock().unwrap().mode = mode;
    }

    pub async fn load(&self) {
        let mut loader = KnownObjectsLoader::new();
        loader.load().await;

        self.0.lock().unwrap().append(loader.objects);
    }

    pub fn append(&self, known_objects: Vec<NONObjectInfo>) {
        self.0.lock().unwrap().append(known_objects)
    }

    pub fn clone_objects(&self) -> Vec<NONObjectInfo> {
        self.0.lock().unwrap().objects.clone()
    }

    pub fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

lazy_static! {
    pub static ref KNOWN_OBJECTS_MANAGER: KnownObjectsManager = KnownObjectsManager::new();
}

use cyfs_base::*;
use cyfs_debug::Mutex;
use cyfs_stack::KnownObject;

use async_std::fs;
use async_std::prelude::*;
use lazy_static::lazy_static;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct KnownObjectsLoader {
    roots: Vec<PathBuf>,
    objects: Vec<KnownObject>,
}

impl KnownObjectsLoader {
    pub fn new() -> Self {
        let desc_folder = cyfs_util::get_cyfs_root_path().join("etc").join("desc");

        Self {
            roots: vec![desc_folder],
            objects: Vec::new(),
        }
    }

    pub async fn load(&mut self) {
        self.scan().await;
    }

    async fn scan(&mut self) {
        let mut i = 0;
        loop {
            if i >= self.roots.len() {
                break;
            }

            let root = self.roots[i].clone();
            let _ = self.scan_root(&root).await;

            i += 1;
        }
    }

    async fn scan_root(&mut self, root: &Path) -> BuckyResult<()> {
        if !root.is_dir() {
            return Ok(());
        }

        let mut entries = fs::read_dir(root).await.map_err(|e| {
            error!(
                "read known object dir failed! dir={}, {}",
                root.display(),
                e
            );
            e
        })?;

        while let Some(res) = entries.next().await {
            let entry = res.map_err(|e| {
                error!("read entry error: {}", e);
                e
            })?;

            let file_path = root.join(entry.file_name());
            if file_path.is_dir() {
                self.roots.push(file_path);
                continue;
            }

            if !file_path.is_file() {
                warn!("path is not file: {}", file_path.display());
                continue;
            }

            if !Self::is_desc_file(&file_path) {
                debug!("not desc file: {}", file_path.display());
                continue;
            }

            if let Ok(ret) = self.load_obj(&file_path).await {
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

    async fn load_obj(&self, file: &Path) -> BuckyResult<KnownObject> {
        let buf = fs::read(file).await.map_err(|e| {
            error!("load known object failed! file={}, {}", file.display(), e);
            e
        })?;

        let (object, _) = AnyNamedObject::raw_decode(&buf).map_err(|e| {
            let msg = format!(
                "invalid known object body buffer: file={}, {}",
                file.display(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::InvalidFormat, msg)
        })?;

        let object_id = object.calculate_id();
        info!(
            "find known object: file={}, object={}",
            file.display(),
            object_id
        );

        let ret = KnownObject {
            object_id,
            object: Arc::new(object),
            object_raw: buf,
        };

        Ok(ret)
    }
}

pub(crate) struct KnownObjectsManagerImpl {
    objects: Vec<KnownObject>,
}

impl KnownObjectsManagerImpl {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    pub async fn load(&mut self) {
        let mut loader = KnownObjectsLoader::new();
        loader.load().await;

        self.append(loader.objects);
    }

    pub fn append(&mut self, known_objects: Vec<KnownObject>) {
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

    pub async fn load(&self) {
        let mut loader = KnownObjectsLoader::new();
        loader.load().await;

        self.0.lock().unwrap().append(loader.objects);
    }

    pub fn append(&self, known_objects: Vec<KnownObject>) {
        self.0.lock().unwrap().append(known_objects)
    }

    pub fn clone_objects(&self) -> Vec<KnownObject> {
        self.0.lock().unwrap().objects.clone()
    }

    pub fn into_objects(self) -> Vec<KnownObject> {
        let mut ret = Vec::new();
        ret.append(&mut self.0.lock().unwrap().objects);

        ret
    }

    pub fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

lazy_static! {
    pub static ref KNOWN_OBJECTS_MANAGER: KnownObjectsManager = KnownObjectsManager::new();
}

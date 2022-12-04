use cyfs_base::*;

use std::fs;
use std::path::{Path, PathBuf};

pub struct DirObjectsSyncLoader {
    roots: Vec<PathBuf>,
    objects: Vec<(PathBuf, Vec<u8>)>,
}

impl DirObjectsSyncLoader {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            roots: vec![root.into()],
            objects: Vec::new(),
        }
    }

    pub fn into_objects(self) -> Vec<(PathBuf, Vec<u8>)> {
        self.objects
    }

    pub fn load(&mut self) {
        let mut i = 0;
        loop {
            if i >= self.roots.len() {
                break;
            }

            let root = self.roots[i].clone();
            let _ = self.scan_root(&root);

            i += 1;
        }
    }

    fn scan_root(&mut self, root: &Path) -> BuckyResult<()> {
        if !root.is_dir() {
            return Ok(());
        }

        let mut entries = fs::read_dir(root).map_err(|e| {
            error!(
                "read object dir failed! dir={}, {}",
                root.display(),
                e
            );
            e
        })?;

        while let Some(res) = entries.next() {
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

            if let Ok(ret) = self.load_file(&file_path) {
                self.objects.push((file_path, ret));
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

    fn load_file(&self, file: &Path) -> BuckyResult<Vec<u8>> {
        let buf = fs::read(file).map_err(|e| {
            error!("load object from file failed! file={}, {}", file.display(), e);
            e
        })?;

        Ok(buf)
    }
}
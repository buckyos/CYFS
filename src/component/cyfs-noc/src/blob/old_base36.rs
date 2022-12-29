use cyfs_base::*;

use std::path::{Path, PathBuf};

trait ToOldBase36 {
    fn to_old_base36(&self) -> String;
}

const ALPHABET: &[u8] = b"0123456789abcdefghijklmnoqprstuvwxyz";

impl ToOldBase36 for [u8] {
    fn to_old_base36(&self) -> String {
        base_x::encode(ALPHABET, self)
    }
}

pub(super) struct FileBlobStorageUpgrade {
    root: PathBuf,
}

impl FileBlobStorageUpgrade {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn get_old_full_path(&self, object_id: &ObjectId) -> PathBuf {
        let hash_str = object_id.as_slice().to_old_base36();
        let len = 3;

        let (tmp, first) = hash_str.split_at(hash_str.len() - len);
        let (_, second) = tmp.split_at(tmp.len() - len);
        self.root.join(format!("{}/{}/{}", first, second, hash_str))
    }

    pub fn try_update(&self, dest: &Path, object_id: &ObjectId) -> bool {
        let old_path = self.get_old_full_path(object_id);
        if !old_path.exists() {
            return false;
        }

        info!(
            "will update blob file for error base36's bug! object={}, {} -> {}",
            object_id,
            old_path.display(),
            dest.display()
        );

        let dir = dest.parent().unwrap();
        if !dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                error!(
                    "create dir for object blob error! path={}, {}",
                    dir.display(),
                    e
                );
            }
        }

        if let Err(e) = std::fs::rename(&old_path, &dest) {
            error!(
                "move blob file error! object={}, {} -> {}, {}",
                object_id,
                old_path.display(),
                dest.display(),
                e,
            );
            return false;
        }

        true
    }
}

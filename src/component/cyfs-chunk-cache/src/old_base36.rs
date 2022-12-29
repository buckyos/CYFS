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

pub(super) struct ChunkStorageUpgrade {
    root: PathBuf,
}

impl ChunkStorageUpgrade {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn get_old_full_path(&self, chunk_id: &ChunkId) -> PathBuf {
        let hash_str = chunk_id.as_slice().to_old_base36();
        let len = 3;

        let (tmp, first) = hash_str.split_at(hash_str.len() - len);
        let (left, second) = tmp.split_at(tmp.len() - len);
        self.root.join(format!("{}/{}/{}", first, second, left))
    }

    pub fn try_update(&self, dest: &Path, chunk_id: &ChunkId) -> bool {
        let old_path = self.get_old_full_path(chunk_id);
        if !old_path.exists() {
            return false;
        }

        log::info!(
            "will update chunk file for error base36's bug! chunk={}, {} -> {}",
            chunk_id,
            old_path.display(),
            dest.display()
        );

        let dir = dest.parent().unwrap();
        if !dir.exists() {
            if let Err(e) = std::fs::create_dir_all(&dir) {
                log::error!(
                    "create dir for chunk error! path={}, {}",
                    dir.display(),
                    e
                );
            }
        }

        if let Err(e) = std::fs::rename(&old_path, &dest) {
            log::error!(
                "move chunk file error! object={}, {} -> {}, {}",
                chunk_id,
                old_path.display(),
                dest.display(),
                e,
            );
            return false;
        }

        true
    }
}

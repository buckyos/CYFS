mod blob;
mod file;
mod old_base36;

pub use blob::*;
pub use file::*;

use cyfs_base::*;
use std::path::Path;

pub async fn create_blob_storage(root: &Path) -> BuckyResult<Box<dyn BlobStorage>> {
    let dir = root.join("objects");

    if !dir.is_dir() {
        if let Err(e) = std::fs::create_dir_all(&dir) {
            let msg = format!(
                "create noc blob data dir error! dir={}, {}",
                dir.display(),
                e
            );
            error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }
    }

    let blob = FileBlobStorage::new(dir);

    Ok(Box::new(blob))
}
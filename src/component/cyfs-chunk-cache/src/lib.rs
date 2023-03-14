mod cached_file;
mod chunk_cache;
mod chunk_manager;
mod local_chunk_cache;
mod local_file;
mod old_base36;

pub use cached_file::*;
pub use chunk_cache::*;
pub use chunk_manager::*;
pub use cyfs_chunk_lib::*;
pub use local_chunk_cache::*;
pub use local_file::*;

use cyfs_base::*;
use std::path::Path;

pub async fn create_local_chunk_cache(
    data_root: &Path,
    isolate: &str,
) -> BuckyResult<Box<dyn ChunkCache>> {
    let isolate = if isolate.is_empty() {
        "default"
    } else {
        isolate
    };
    let chunk_dir = data_root.join("chunk-cache").join(isolate);

    if !chunk_dir.is_dir() {
        if let Err(e) = std::fs::create_dir_all(&chunk_dir) {
            let msg = format!(
                "create chunk cache local dir error! dir={}, {}",
                chunk_dir.display(),
                e
            );
            log::error!("{}", msg);

            return Err(BuckyError::new(BuckyErrorCode::IoError, msg));
        }
    }

    let cache: SingleDiskChunkCache = SingleDiskChunkCache::new(chunk_dir);

    Ok(Box::new(cache))
}

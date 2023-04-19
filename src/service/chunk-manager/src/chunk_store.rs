use cyfs_base::{BuckyResult, ChunkId};

use std::path::{Path, PathBuf};
use async_std::fs::File;
use std::str::FromStr;
use cyfs_lib::{NDNPutDataRequest, SharedCyfsStack};
use log::*;

#[derive(Debug)]
pub struct ChunkStore {
    chunk_dir: PathBuf,
}

impl ChunkStore {
    pub async fn merge(chunk_dir: &Path, stack: SharedCyfsStack) -> BuckyResult<()> {
        for entry in std::fs::read_dir(chunk_dir)? {
            let entry = entry?;

            if entry.file_type()?.is_file() {
                let path = entry.path();
                let chunk_id = path.file_name().unwrap().to_string_lossy();
                match ChunkId::from_str(&chunk_id) {
                    Ok(id) => {
                        let len = entry.metadata()?.len();
                        let chunk_len = id.len();
                        if len != chunk_len as u64 {
                            error!("chunk {} len mismatch! except {}, actual {}, skip it.", path.display(), chunk_len, len);
                            continue;
                        }
                        let data = File::open(&path).await?;
                        match stack.ndn_service().put_data(NDNPutDataRequest::new_ndc(id.object_id(), len, Box::new(data))).await {
                            Ok(resp) => {
                                info!("insert chunk {} into stack result {}", id, resp.result.to_string());
                                Ok(())
                            }
                            Err(e) => {
                                error!("insert chunk {} into stack err {}", id, e);
                                Err(e)
                            }
                        }?;
                    }
                    Err(e) => {
                        error!("invalid chunk file {}, decode chunk id err {}. skip it", path.display(), e)
                    }
                }
            }
        }

        info!("merge success, delete old chunk folder {}", chunk_dir.display());
        let _ = std::fs::remove_dir_all(&chunk_dir);

        Ok(())
    }
}
use cyfs_base::*;
use cyfs_util::*;

use std::sync::Arc;
use std::path::PathBuf;

pub struct ChunkTrackerFixer {
    tracker: TrackerCacheRef,
}

impl ChunkTrackerFixer {
    pub fn new(isolate: &str) -> BuckyResult<Self> {
        let tracker = Self::init_tracker(isolate)?;
        let ret = Self {
            tracker: Arc::new(tracker),
        };

        Ok(ret)
    }

    pub fn init_tracker(isolate: &str) -> BuckyResult<Box<dyn TrackerCache>> {
        use cyfs_tracker_cache::TrackerCacheManager;

        TrackerCacheManager::create_tracker_cache(isolate)
    }

    pub async fn try_fix_chunk_pos(&self, chunk_id: &ChunkId) -> BuckyResult<()> {
        let ret = self.check_chunk_pos(chunk_id).await?;
        if ret {
            return Ok(());
        }

        let req = AddTrackerPositonRequest {
            id: chunk_id.to_string(),
            direction: TrackerDirection::Store,
            pos: TrackerPostion::ChunkManager,
            flags: 0,
        };

        self.tracker.add_position(&req).await.map_err(|e| {
            let msg = format!(
                "record chunk with pos in chunk manager failed! chunk={}, {}",
                chunk_id, e
            );
            error!("{}", msg);
            BuckyError::new(e.code(), msg)
        })
    }

    async fn check_chunk_pos(&self, chunk_id: &ChunkId) -> BuckyResult<bool> {
        let request = GetTrackerPositionRequest {
            id: chunk_id.to_string(),
            direction: Some(TrackerDirection::Store),
        };

        let ret = self.tracker.get_position(&request).await?;
        if ret.len() == 0 {
            warn!("chunk not eixsts in tracker: chunk={}", chunk_id);
            return Ok(false);
        }

        for c in ret {
            let mut need_delete = false;
            let mut exists_in_chunk_manager = false;

            match &c.pos {
                TrackerPostion::File(path) => {
                    let file = PathBuf::from(path);
                    if !file.exists() {
                        warn!(
                            "chunk's storage file not exists! chunk={}, file={}",
                            chunk_id, path
                        );
                        need_delete = true;
                    } else {
                        // FIXME should check the content? It will be verified once when the chunk is actually read
                    }
                }
                TrackerPostion::FileRange(fr) => {
                    let file = PathBuf::from(&fr.path);
                    if !file.exists() {
                        warn!(
                            "chunk's storage file not exists! chunk={}, file={}",
                            chunk_id, fr.path
                        );
                        need_delete = true;
                    } else {
                        // FIXME should check the content? It will be verified once when the chunk is actually read
                    }
                }
                TrackerPostion::ChunkManager => {
                    exists_in_chunk_manager = true;
                }
                value @ _ => {
                    let msg = format!(
                        "unsupport tracker postion for chunk={}, position={:?}",
                        chunk_id, value,
                    );
                    error!("{}", msg);
                }
            };

            if need_delete {
                // try delete this tracker record if relate file not exists!
                let _ = self
                    .tracker
                    .remove_position(&RemoveTrackerPositionRequest {
                        id: chunk_id.to_string(),
                        direction: Some(TrackerDirection::Store),
                        pos: Some(c.pos.clone()),
                    })
                    .await;
            }

            if exists_in_chunk_manager {
                return Ok(true);
            }
        }

        warn!("chunk not exists in chunk manager: {}", chunk_id);
        Ok(false)
    }
}

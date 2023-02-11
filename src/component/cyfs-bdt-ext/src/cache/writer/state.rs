use cyfs_base::*;
use cyfs_util::cache::*;

use std::{collections::HashMap, path::PathBuf};

pub struct FileStateUpdater;

impl FileStateUpdater {
    pub async fn add_file_to_ndc(
        ndc: &Box<dyn NamedDataCache>,
        file: &File,
        dirs: Option<Vec<FileDirRef>>,
    ) -> BuckyResult<()> {
        let file_id = file.desc().file_id();
        let file_req = InsertFileRequest {
            file_id: file_id.clone(),
            file: file.to_owned(),
            flags: 0,
            quick_hash: None,
            dirs,
        };

        ndc.insert_file(&file_req).await.map_err(|e| {
            error!("record file to ndc error! file={}, {}", file_id, e);
            e
        })?;

        info!("record file to ndc success! file={}", file_id,);

        Ok(())
    }
}

struct ChunkStateUpdater;

impl ChunkStateUpdater {
    pub async fn update_chunk_state(
        ndc: &Box<dyn NamedDataCache>,
        chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        let req = InsertChunkRequest {
            chunk_id: chunk_id.clone(),
            state: ChunkState::Ready,
            ref_objects: None,
            trans_sessions: None,
            flags: 0,
        };

        ndc.insert_chunk(&req).await.map_err(|e| {
            error!(
                "insert and update chunk state error! chunk={}, {}",
                chunk_id, e
            );
            e
        })?;

        Ok(())
    }
}

pub struct ChunkManagerStateUpdater;

impl ChunkManagerStateUpdater {
    pub async fn update_chunk_state(
        ndc: &Box<dyn NamedDataCache>,
        chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        ChunkStateUpdater::update_chunk_state(ndc, chunk_id).await
    }

    pub async fn update_chunk_tracker(
        tracker: &Box<dyn TrackerCache>,
        chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        let request = AddTrackerPositonRequest {
            id: chunk_id.to_string(),
            direction: TrackerDirection::Store,
            pos: TrackerPostion::ChunkManager,
            flags: 0,
        };
        if let Err(e) = tracker.add_position(&request).await {
            if e.code() != BuckyErrorCode::AlreadyExists {
                error!("add chunk to tracker failed! chunk={}, {}", chunk_id, e);
                return Err(e);
            }
        };

        Ok(())
    }
}

pub struct LocalFileStateUpdater {
    file: File,
    local_path: PathBuf,
    chunk_map: HashMap<ChunkId, Vec<(u64, u64)>>,
}

impl LocalFileStateUpdater {
    pub fn new(file: File, local_path: PathBuf) -> Self {
        let mut chunk_map = HashMap::new();
        if let Some(chunk_list) = file
            .body()
            .as_ref()
            .unwrap()
            .content()
            .chunk_list()
            .inner_chunk_list()
        {
            let mut pos = 0;
            for chunk_id in chunk_list.iter() {
                if !chunk_map.contains_key(chunk_id) {
                    chunk_map.insert(chunk_id.clone(), vec![(pos as u64, chunk_id.len() as u64)]);
                } else {
                    chunk_map
                        .get_mut(chunk_id)
                        .unwrap()
                        .push((pos as u64, chunk_id.len() as u64));
                }
                pos += chunk_id.len();
            }
        }

        Self {
            file,
            local_path,
            chunk_map,
        }
    }

    pub async fn update_chunk_state(
        &self,
        ndc: &Box<dyn NamedDataCache>,
        chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        ChunkStateUpdater::update_chunk_state(ndc, chunk_id).await
    }

    pub async fn update_chunk_tracker(
        &self,
        tracker: &Box<dyn TrackerCache>,
        chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        let chunk_range_list = self.get_chunk_range_list(chunk_id)?;

        let id = chunk_id.to_string();
        for (offset, length) in chunk_range_list.iter() {
            let request = AddTrackerPositonRequest {
                id: id.clone(),
                direction: TrackerDirection::Store,
                pos: TrackerPostion::FileRange(PostionFileRange {
                    path: self.local_path.to_string_lossy().to_string(),
                    range_begin: *offset,
                    range_end: *offset + *length,
                }),
                flags: 0,
            };
            if let Err(e) = tracker.add_position(&request).await {
                if e.code() != BuckyErrorCode::AlreadyExists {
                    error!("add to tracker failed for {}", e);
                    return Err(e);
                }
            };
        }

        Ok(())
    }

    fn get_chunk_range_list(&self, chunk_id: &ChunkId) -> BuckyResult<&Vec<(u64, u64)>> {
        match self.chunk_map.get(chunk_id) {
            Some(range_list) => Ok(range_list),
            None => Err(BuckyError::new(
                BuckyErrorCode::NotFound,
                format!(
                    "chunk {} not found in {}",
                    chunk_id.to_string(),
                    self.local_path.to_string_lossy().to_string()
                ),
            )),
        }
    }
}

pub struct LocalChunkStateUpdater {
    local_path: PathBuf,
}

impl LocalChunkStateUpdater {
    pub fn new(local_path: PathBuf) -> Self {
        Self { local_path }
    }

    pub async fn update_chunk_state(
        &self,
        ndc: &Box<dyn NamedDataCache>,
        chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        ChunkStateUpdater::update_chunk_state(ndc, chunk_id).await
    }

    pub async fn update_chunk_tracker(
        &self,
        tracker: &Box<dyn TrackerCache>,
        chunk_id: &ChunkId,
    ) -> BuckyResult<()> {
        let request = AddTrackerPositonRequest {
            id: chunk_id.to_string(),
            direction: TrackerDirection::Store,
            pos: TrackerPostion::File(self.local_path.to_string_lossy().to_string()),
            flags: 0,
        };
        if let Err(e) = tracker.add_position(&request).await {
            if e.code() != BuckyErrorCode::AlreadyExists {
                error!("chunk add to tracker failed! chunk={}, {}", chunk_id, e);
                return Err(e);
            }
        };

        Ok(())
    }
}

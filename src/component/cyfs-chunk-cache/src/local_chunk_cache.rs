use std::fs::{create_dir_all};
use std::hash::Hasher;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::u64;
use cyfs_base::*;
use sysinfo::{DiskExt, RefreshKind, SystemExt};
use crate::old_base36::ChunkStorageUpgrade;
use crate::{Chunk, ChunkCache, ChunkMut, MMapChunk, MMapChunkMut, MemChunk, ChunkType};
use num_traits::float::Float;
use futures_lite::AsyncWriteExt;
use num_traits::abs;
use scan_dir::ScanDir;
use cyfs_chunk_lib::ChunkMeta;
use cyfs_debug::Mutex;

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) struct LocalChunkCacheMetaRecord {
    pub prev_hash: Option<HashValue>,
    pub list: Vec<(String, u32)>,
}

impl LocalChunkCacheMetaRecord {
    pub fn new(prev_hash: Option<HashValue>, list: Vec<(String, u32)>) -> Self {
        Self { prev_hash, list }
    }

    pub fn hash(&self) -> HashValue {
        hash_data(self.to_vec().unwrap().as_slice())
    }

    pub fn is_same(&self, disk_cache_list: &Vec<(String, u32)>) -> bool {
        if self.list.len() != disk_cache_list.len() {
            false
        } else {
            for (last_item_path, last_item_weight) in self.list.iter() {
                let mut find = false;
                for (new_item_path, new_item_weight) in disk_cache_list.iter() {
                    if new_item_path == last_item_path && *new_item_weight == *last_item_weight {
                        find = true;
                        break;
                    }
                }

                if !find {
                    return false;
                }
            }
            true
        }
    }

    pub fn get_weight(&self, path: &str) -> Option<u32> {
        for (item, weight) in self.list.iter() {
            if path == item.as_str() {
                return Some(*weight)
            }
        }
        None
    }
}

#[derive(Clone, RawEncode, RawDecode)]
pub(crate) struct LocalChunkCacheMeta {
    list: Vec<LocalChunkCacheMetaRecord>,
}

impl LocalChunkCacheMeta {
    pub fn new() -> Self {
        Self { list: vec![] }
    }

    pub fn add_update_record(&mut self, disk_cache_list: Vec<(String, u32)>) {
        let record = match self.list.last() {
            Some(last) => {
                LocalChunkCacheMetaRecord::new(Some(last.hash()), disk_cache_list)
            },
            None => {
                LocalChunkCacheMetaRecord::new(None, disk_cache_list)
            }
        };
        self.list.push(record);
    }

    pub fn last_hash(&self) -> Option<HashValue> {
        match self.list.last() {
            Some(last) => {
                last.prev_hash.clone()
            },
            None => None
        }
    }

    pub fn is_latest(&self, disk_cache_list: &Vec<(String, u32)>) -> bool {
        match self.list.last() {
            Some(last) => {
                last.is_same(disk_cache_list)
            },
            None => false
        }
    }

    pub fn get_latest_weight(&self, path: &str) -> Option<u32> {
        match self.list.last() {
            Some(last) => {
                last.get_weight(path)
            },
            None => None
        }
    }

    pub fn record_count(&self) -> usize {
        self.list.len()
    }

    pub fn get_latest_record(&self) -> Option<LocalChunkCacheMetaRecord> {
        match self.list.last() {
            Some(last) => {
                Some(last.clone())
            },
            None => None
        }
    }

    pub fn get_record(&self, index: usize) -> Option<&LocalChunkCacheMetaRecord> {
        self.list.get(index)
    }
}

pub(crate) fn get_cache_path_list() -> Vec<(PathBuf, u64)> {
    let system = sysinfo::System::new_with_specifics(RefreshKind::new().with_disks_list());
    let disk_list = system.disks();
    let mut cache_list = Vec::new();
    for disk in disk_list {
        let path = disk.mount_point().join("cyfs").join("chunk_cache");
        if !path.exists() {
            let _ = create_dir_all(path.as_path());
        }

        cache_list.push((path, disk.available_space()));
    }
    cache_list
}

pub(crate) fn get_disk_info_of_path(path: &Path) -> (u64, u64){
    let system = sysinfo::System::new_with_specifics(RefreshKind::new().with_disks_list());
    let dist_list = system.disks();
    let mut total = 0;
    let mut available = 0;
    let mut max_match_path = PathBuf::new();
    for disk in dist_list {
        let disk_mount = disk.mount_point().to_path_buf();
        if path.starts_with(disk_mount.as_path()) && max_match_path.to_string_lossy().to_string().len() < disk_mount.to_string_lossy().to_string().len() {
            max_match_path = disk_mount;
            total = disk.total_space();
            available = disk.available_space();
        }
    }
    (total, available)
}

pub(crate) async fn get_path_size(path: PathBuf) -> BuckyResult<u64> {
    async_std::task::spawn_blocking(move || {
        ScanDir::files().skip_dirs(true).walk(path.as_path(), |it| {
            let mut sum = 0;
            for (entry, _) in it {
                match entry.metadata() {
                    Ok(meta) => sum += meta.len(),
                    Err(_) => {}
                }
            }
            sum
        }).map_err(|_e| {
            let msg = format!("scan path {} err", path.to_string_lossy().to_string());
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })
    }).await
}

pub(crate) trait DiskScanner: Send + Sync {
    fn get_cache_path_list(&self) -> Vec<(PathBuf, u64)>;
}

pub(crate) struct DefaultDiskScanner;

impl DiskScanner for DefaultDiskScanner {
    fn get_cache_path_list(&self) -> Vec<(PathBuf, u64)> {
        get_cache_path_list()
    }
}

pub(crate) struct LocalChunkCache<CACHE: TSingleDiskChunkCache + ChunkCache, SCANNER: DiskScanner> {
    disk_cache_list: RwLock<Vec<(Arc<CACHE>, u32)>>,
    cache_meta: Mutex<LocalChunkCacheMeta>,
    scanner: SCANNER,
    isolate: String,
}

impl <CACHE: TSingleDiskChunkCache + ChunkCache, SCANNER: DiskScanner> LocalChunkCache<CACHE, SCANNER> {
    pub async fn new(isolate: &str, scanner: SCANNER) -> BuckyResult<Self> {
        let obj = Self {
            disk_cache_list: RwLock::new(Vec::new()),
            cache_meta: Mutex::new(LocalChunkCacheMeta::new()),
            scanner,
            isolate: if isolate.is_empty() { "default".to_string() } else { isolate.to_string() }
        };
        obj.refresh_cache().await?;
        Ok(obj)
    }

    fn get_cache(&self, path: &Path) -> Option<Arc<CACHE>> {
        let cache_list = self.disk_cache_list.read().unwrap();
        for (cache, _) in cache_list.iter() {
            if cache.get_cache_path() == path {
                return Some(cache.clone());
            }
        }
        None
    }

    pub async fn refresh_cache(&self) -> BuckyResult<()> {
        let mut disk_cache_list = Vec::new();
        let mut latest_meta = None;
        let mut max_record_count = -1;
        // 遍历磁盘列表
        let cache_list = self.scanner.get_cache_path_list();
        for (path, space) in cache_list.iter() {
            log::info!("read cache path {} {}", path.to_string_lossy().to_string(), *space);
            let path = path.as_path().join(self.isolate.as_str());
            if !path.exists() {
                let _ = create_dir_all(path.as_path());
            }
            let cache = match self.get_cache(path.as_path()) {
                Some(cache) => {
                    cache
                },
                None => {
                    Arc::new(CACHE::new(path.to_path_buf()))
                }
            };
            let cache_meta = cache.get_local_cache_meta()?;
            let weight = (space/1024/1024/1024) as u32;

            // 获取最新的cache元数据列表
            if cache_meta.record_count() as i32 > max_record_count {
                max_record_count = cache_meta.record_count() as i32;
                latest_meta = Some(cache_meta.clone());
            }
            if weight > 1 {
                disk_cache_list.push((cache, weight));
            }
        }

        if let Some(mut global_last_record) = latest_meta {
            let mut meta_list = Vec::new();
            for (cache, weight) in disk_cache_list.iter_mut() {
                let path_str = cache.get_cache_path().to_string_lossy().to_string();
                if let Some(latest_weight) = global_last_record.get_latest_weight(path_str.as_str()) {
                    //如果当前空间大于上次记录空间，表示该区域已经扩容
                    if *weight > latest_weight {
                        let path_size = (get_path_size(cache.get_cache_path().to_path_buf()).await?/1024/1024/1024) as u32;
                        //如果新扩容区域小于上次记录空间的20%或扩容空间小于50G，则保持全新不变
                        if ((*weight + path_size - latest_weight) as f32 / (latest_weight as f32) < 0.2 ) || (*weight + path_size - latest_weight < 50) {
                            *weight = latest_weight;
                        } else {
                            *weight = *weight + path_size;
                        }
                        //如果当前空间大于10G或当前空间大小大于上次记录空间的10%则保持权重不变
                    } else if *weight > 10 || *weight as f32 / (latest_weight as f32) > 0.1 {
                        *weight = latest_weight;
                    } else {
                        let path_size = (get_path_size(cache.get_cache_path().to_path_buf()).await?/1024/1024/1024) as u32;

                        //如果理论剩余空间和真实剩余空间的比例小于15%或理论剩余空间和真实剩余空间的差值小于5G，则保持权重不变
                        if abs(1f32 - (latest_weight - path_size) as f32 / (*weight as f32)) < 0.15 || abs(latest_weight as i64 - path_size as i64 - *weight as i64) < 5 {
                            *weight = latest_weight;
                        } else {
                            *weight = path_size + *weight;
                        }
                    }
                }

                log::info!("cache path {} weight {}", path_str.as_str(), *weight);
                meta_list.push((path_str, *weight));
            }

            if !global_last_record.is_latest(&meta_list) {
                global_last_record.add_update_record(meta_list);
                log::info!("chunk cache change.item count {}", global_last_record.record_count());

                for (cache, _) in disk_cache_list.iter_mut() {
                    cache.set_local_cache_meta(&global_last_record)?;
                }
            }
            *self.cache_meta.lock().unwrap() = global_last_record;
        }

        let mut cur_cache_list = self.disk_cache_list.write().unwrap();
        *cur_cache_list = disk_cache_list;

        Ok(())
    }

    fn hash(chunk_id: &ChunkId, cache_id: &HashValue) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        hasher.write(chunk_id.as_slice());
        hasher.write(cache_id.as_slice());
        hasher.finish()
    }

    fn alloc_disk_cache(&self, chunk_id: &ChunkId) -> BuckyResult<Arc<CACHE>> {
        let mut max = f64::MIN;
        let mut cache = None;
        let disk_cache_list = self.disk_cache_list.read().unwrap();
        if disk_cache_list.len() == 1 {
            return Ok(disk_cache_list.get(0).as_ref().unwrap().0.clone());
        }
        for (disk_cache, weight) in disk_cache_list.iter() {
            let hash = Self::hash(chunk_id, disk_cache.get_cache_id());
            let v = (hash as f64/u64::MAX as f64).ln() / (*weight as f64);
            if v > max {
                max = v;
                cache = Some(disk_cache);
            }
        }
        if cache.is_some() {
            Ok(cache.unwrap().clone())
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "not alloc chunk cache"))
        }
    }

    fn get_disk_cache(&self, chunk_id: &ChunkId) -> BuckyResult<Arc<CACHE>> {
        let mut max = f64::min_value();
        let mut cache = None;
        let disk_cache_list = self.disk_cache_list.read().unwrap();
        if disk_cache_list.len() == 1 {
            return Ok(disk_cache_list.get(0).as_ref().unwrap().0.clone());
        }
        for (disk_cache, weight) in disk_cache_list.iter() {
            let hash = Self::hash(chunk_id, disk_cache.get_cache_id());
            let v = (hash as f64/u64::MAX as f64).ln() / (*weight as f64);
            if v > max {
                max = v;
                cache = Some(disk_cache);
            }
        }
        if cache.is_some() {
            let cache = cache.unwrap().clone();
            Ok(cache)
        } else {
            Err(BuckyError::new(BuckyErrorCode::NotFound, "not find chunk cache"))
        }
    }

    async fn find_chunk_from_prev_async(&self, chunk_id: &ChunkId, cache: &Arc<CACHE>) -> BuckyResult<()> {
        let cache_meta = self.cache_meta.lock().unwrap().clone();
        let mut index = cache_meta.record_count() as i64 - 2;
        while index >= 0 {
            let record = cache_meta.get_record(index as usize).unwrap();
            let mut max = f64::min_value();
            let mut max_cache = None;
            for (cache_path, weight) in record.list.iter() {
                let tmp_cache = Arc::new(CACHE::new(PathBuf::from(cache_path.to_string())));
                let hash = Self::hash(chunk_id, tmp_cache.get_cache_id());
                let v = (hash as f64/u64::MAX as f64).ln() / (*weight as f64);
                if v > max {
                    max = v;
                    max_cache = Some(tmp_cache);
                }
            }
            if max_cache.is_some() {
                let tmp_cache = max_cache.unwrap();
                if let Ok(chunk) = tmp_cache.get_chunk(chunk_id, ChunkType::MMapChunk).await {
                    cache.put_chunk(chunk_id, chunk.as_ref()).await?;
                    tmp_cache.delete_chunk(chunk_id).await?;
                    return Ok(())
                }
            }
            index -= 1;
        }
        return Err(BuckyError::new(BuckyErrorCode::NotFound, "not find chunk"));
    }

    async fn find_chunk_cache_from_prev(&self, chunk_id: &ChunkId) -> BuckyResult<Arc<CACHE>> {
        let cache_meta = self.cache_meta.lock().unwrap().clone();
        let mut index = cache_meta.record_count() as i64 - 2;
        while index >= 0 {
            let record = cache_meta.get_record(index as usize).unwrap();
            let mut max = f64::min_value();
            let mut max_cache = None;
            for (cache_path, weight) in record.list.iter() {
                let tmp_cache = Arc::new(CACHE::new(PathBuf::from(cache_path.to_string())));
                let hash = Self::hash(chunk_id, tmp_cache.get_cache_id());
                let v = (hash as f64/u64::MAX as f64).ln() / (*weight as f64);
                if v > max {
                    max = v;
                    max_cache = Some(tmp_cache);
                }
            }
            if max_cache.is_some() {
                let tmp_cache = max_cache.unwrap();
                if tmp_cache.is_exist(chunk_id).await {
                    return Ok(tmp_cache)
                }
            }
            index -= 1;
        }
        return Err(BuckyError::new(BuckyErrorCode::NotFound, "not find chunk"));
    }
}

#[async_trait::async_trait]
impl <CACHE: TSingleDiskChunkCache + ChunkCache, SCANNER: DiskScanner> ChunkCache for LocalChunkCache<CACHE, SCANNER> {
    async fn get_chunk(&self, chunk_id: &ChunkId, chunk_type: ChunkType) -> BuckyResult<Box<dyn Chunk>> {
        let cache = self.get_disk_cache(chunk_id)?;
        match cache.get_chunk(chunk_id, chunk_type).await {
            Ok(chunk) => {
                Ok(chunk)
            },
            Err(e) => {
                if e.code() == BuckyErrorCode::NotFound {
                    self.find_chunk_from_prev_async(chunk_id, &cache).await?;
                    cache.get_chunk(chunk_id, chunk_type).await
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn new_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<Box<dyn ChunkMut>> {
        let cache = self.alloc_disk_cache(chunk_id)?;
        cache.new_chunk(chunk_id).await
    }

    async fn delete_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<()> {
        let cache = self.get_disk_cache(chunk_id)?;
        if cache.is_exist(chunk_id).await {
            cache.delete_chunk(chunk_id).await
        } else {
            if let Ok(cache) = self.find_chunk_cache_from_prev(chunk_id).await {
                cache.delete_chunk(chunk_id).await
            } else {
                Ok(())
            }
        }
    }

    async fn put_chunk(&self, chunk_id: &ChunkId, chunk: &dyn Chunk) -> BuckyResult<()> {
        let cache = self.alloc_disk_cache(chunk_id)?;
        cache.put_chunk(chunk_id, chunk).await
    }

    async fn is_exist(&self, chunk_id: &ChunkId) -> bool {
        let cache = match self.get_disk_cache(chunk_id) {
            Ok(cache) => cache,
            Err(_) => return false
        };
        if !cache.is_exist(chunk_id).await {
            match self.find_chunk_from_prev_async(chunk_id, &cache).await {
                Ok(()) => true,
                Err(_) => false
            }
        } else {
            true
        }
    }

    async fn get_chunk_meta(&self, chunk_id: &ChunkId, chunk_type: ChunkType) -> BuckyResult<ChunkMeta> {
        let cache = self.get_disk_cache(chunk_id)?;
        match cache.get_chunk_meta(chunk_id, chunk_type).await {
            Ok(chunk) => {
                Ok(chunk)
            },
            Err(e) => {
                if e.code() == BuckyErrorCode::NotFound {
                    self.find_chunk_from_prev_async(chunk_id, &cache).await?;
                    cache.get_chunk_meta(chunk_id, chunk_type).await
                } else {
                    Err(e)
                }
            }
        }
    }
}

pub(crate) trait TSingleDiskChunkCache {
    fn new(path: PathBuf) -> Self;
    fn get_cache_id(&self) -> &HashValue;
    fn get_cache_path(&self) -> &Path;
    fn get_local_cache_meta(&self) -> BuckyResult<LocalChunkCacheMeta>;
    fn set_local_cache_meta(&self, meta: &LocalChunkCacheMeta) -> BuckyResult<()>;
}

pub(crate) struct SingleDiskChunkCache {
    path: PathBuf,
    cache_id: HashValue,

    #[cfg(target_os = "windows")]
    upgrade: super::old_base36::ChunkStorageUpgrade,
}

impl SingleDiskChunkCache {
    fn get_disk_free_space(&self) -> u64 {
        0
    }

    fn get_file_path(&self, file_id: &ChunkId, is_create: bool) -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            let hash_str = file_id.to_base36();
            let (tmp, last) = hash_str.split_at(hash_str.len() - 3);
            let (first, mid) = tmp.split_at(tmp.len() - 3);
            let path = self.path.join(last).join(mid);
            if is_create && !path.exists() {
                let _ = create_dir_all(path.as_path());
            }
            path.join(first)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let hash_str = file_id.to_string();
            let (tmp, last) = hash_str.split_at(hash_str.len() - 2);
            let (first, mid) = tmp.split_at(tmp.len() - 2);
            let path = self.path.join(last).join(mid);
            if is_create && !path.exists() {
                let _ = create_dir_all(path.as_path());
            }
            path.join(first)
        }
    }

    pub async fn remove_file(&self, file_id: &ChunkId) -> BuckyResult<()> {
        let file_path = self.get_file_path(file_id, false);
        let _ = async_std::fs::remove_file(file_path.as_path()).await.map_err(|e| {
            log::error!("remove file {} failed.err={}", file_path.to_string_lossy().to_string(), &e);
            BuckyError::from(e)
        });
        Ok(())
    }

    fn chunk_exist(&self, chunk_id: &ChunkId) -> bool {
        let file_path = self.get_file_path(chunk_id, false);
        if !file_path.exists() {
            return false;
        }

        let file_meta = match std::fs::metadata(file_path.as_path()) {
            Ok(meta) => meta,
            Err(e) => {
                let msg = format!("read file {} meta err.{}", file_path.to_string_lossy().to_string(), e);
                log::error!("{}", msg);
                return false;
            }
        };

        if chunk_id.len() as u64 != file_meta.len() {
            false
        } else {
            true
        }
    }
}

impl TSingleDiskChunkCache for SingleDiskChunkCache {
    fn new(path: PathBuf) -> Self {
        let cache_id = hash_data(path.to_string_lossy().to_string().as_bytes());
        Self {
            #[cfg(target_os = "windows")]
            upgrade: ChunkStorageUpgrade::new(path.clone()),

            path,
            cache_id,
        }
    }

    fn get_cache_id(&self) -> &HashValue {
        &self.cache_id
    }

    fn get_cache_path(&self) -> &Path {
        self.path.as_path()
    }

    fn get_local_cache_meta(&self) -> BuckyResult<LocalChunkCacheMeta> {
        let meta_path = self.path.join("cache.meta");
        if !meta_path.exists() {
            return Ok(LocalChunkCacheMeta::new());
        }
        let (meta, _) = LocalChunkCacheMeta::decode_from_file(meta_path.as_path(), &mut Vec::new())?;
        Ok(meta)
    }

    fn set_local_cache_meta(&self, meta: &LocalChunkCacheMeta) -> BuckyResult<()> {
        let meta_path = self.path.join("cache.meta");
        meta.encode_to_file(meta_path.as_path(), false)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl ChunkCache for SingleDiskChunkCache {
    async fn get_chunk(&self, chunk_id: &ChunkId, chunk_type: ChunkType) -> BuckyResult<Box<dyn Chunk>> {
        log::info!("SingleDiskChunkCache get_chunk {}", chunk_id.to_string());
        let file_path = self.get_file_path(chunk_id, false);
        if !file_path.exists() {
            #[cfg(target_os = "windows")]
            {
                if !self.upgrade.try_update(&file_path, chunk_id) {
                    let msg = format!("get chunk's file but not exist! chunk={}, file={}", chunk_id, file_path.display());
                    log::warn!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
                }
            }
            #[cfg(not(target_os = "windows"))]
            {
                let msg = format!("get chunk's file but not exist! chunk={}, file={}", chunk_id, file_path.display());
                log::warn!("{}", msg);
                return Err(BuckyError::new(BuckyErrorCode::NotFound, msg));
            }
        }

        match chunk_type {
            ChunkType::MMapChunk => {
                let chunk: Box<dyn Chunk> = Box::new(MMapChunk::open(file_path, None).await?);
                Ok(chunk)
            },
            ChunkType::MemChunk => {
                let buf = async_std::fs::read(file_path.as_path()).await.map_err(|e| {
                    let msg = format!("open chunk's file error! chunk={}, file={}, {}", chunk_id, file_path.display(), e);
                    log::error!("{}", msg);
                    BuckyError::new(BuckyErrorCode::IoError, msg)
                })?;
                let chunk: Box<dyn Chunk> = Box::new(MemChunk::from(buf));
                Ok(chunk)
            }
        }
    }

    async fn new_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<Box<dyn ChunkMut>> {
        let file_path = self.get_file_path(chunk_id, true);
        log::info!("new chunk {}", file_path.to_string_lossy().to_string());
        if file_path.exists() {
            let msg = format!("[{}:{}] file {} exist", file!(), line!(), file_path.to_string_lossy().to_string());
            log::error!("{}", msg.as_str());
            return Err(BuckyError::new(BuckyErrorCode::AlreadyExists, msg));
        }

        let len = chunk_id.len();
        let chunk = MMapChunkMut::open(file_path, len as u64, None).await?;
        Ok(Box::new(chunk))
    }

    async fn delete_chunk(&self, chunk_id: &ChunkId) -> BuckyResult<()> {
        let file_path = self.get_file_path(chunk_id, false);
        if file_path.exists() {
            let _ = async_std::fs::remove_file(file_path).await;
        }
        Ok(())
    }

    async fn put_chunk(&self, chunk_id: &ChunkId, chunk: &dyn Chunk) -> BuckyResult<()> {
        assert_eq!(chunk_id.len(), chunk.get_len());
        let file_path = self.get_file_path(chunk_id, true);
        log::info!("put chunk {}", file_path.to_string_lossy().to_string());
        if file_path.exists() {
            let msg = format!("[{}:{}] file {} exist", file!(), line!(), file_path.to_string_lossy().to_string());
            log::info!("{}", msg.as_str());
            return Ok(());
        }

        let mut file = async_std::fs::OpenOptions::new().read(true).write(true).create(true).open(file_path.as_path()).await.map_err(|e| {
            let msg = format!("[{}:{}] open {} failed.err {}", file!(), line!(), file_path.to_string_lossy().to_string(), e);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;

        file.write_all(&chunk[..chunk.get_len()]).await.map_err(|e| {
            let msg = format!("[{}:{}] write {} failed.err {}", file!(), line!(), file_path.to_string_lossy().to_string(), e);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;
        file.flush().await.map_err(|e| {
            let msg = format!("[{}:{}] flush {} failed.err {}", file!(), line!(), file_path.to_string_lossy().to_string(), e);
            log::error!("{}", msg.as_str());
            BuckyError::new(BuckyErrorCode::Failed, msg)
        })?;
        log::info!("put chunk {} complete", file_path.to_string_lossy().to_string());
        Ok(())
    }

    async fn is_exist(&self, chunk_id: &ChunkId) -> bool {
        self.chunk_exist(chunk_id)
    }

    async fn get_chunk_meta(&self, chunk_id: &ChunkId, chunk_type: ChunkType) -> BuckyResult<ChunkMeta> {
        log::info!("SingleDiskChunkCache get_chunk {}", chunk_id.to_string());
        let file_path = self.get_file_path(chunk_id, false);
        if !file_path.exists() {
            return Err(BuckyError::new(BuckyErrorCode::NotFound, format!("[{}:{}] file {} not exist", file!(), line!(), file_path.to_string_lossy().to_string())));
        }

        match chunk_type {
            ChunkType::MMapChunk => {
                Ok(ChunkMeta::MMapChunk(file_path.to_string_lossy().to_string(), None))
            },
            ChunkType::MemChunk => {
                let buf = async_std::fs::read(file_path.as_path()).await.map_err(|e| {
                    let msg = format!("[{}:{}] open {} failed.err {}", file!(), line!(), file_path.to_string_lossy().to_string(), e);
                    log::error!("{}", msg.as_str());
                    BuckyError::new(BuckyErrorCode::Failed, msg)
                })?;
                Ok(ChunkMeta::MemChunk(buf))
            }
        }
    }
}

#[cfg(test)]
mod test_local_chunk_cache {
    use std::collections::HashMap;
    use std::io::{SeekFrom, Write};
    use std::ops::{Deref, DerefMut};
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;
    use futures_lite::{AsyncReadExt, AsyncWriteExt};
    use cyfs_chunk_lib::ChunkMeta;
    use cyfs_base::{BuckyError, BuckyErrorCode, BuckyResult, ChunkId, hash_data, HashValue};
    use crate::{ChunkCache, Chunk, ChunkMut, ChunkType, DiskScanner, LocalChunkCache, LocalChunkCacheMeta, TSingleDiskChunkCache, ChunkRead, ChunkWrite};

    pub type RandomId = HashValue;

    pub trait TRandomId {
        fn new_id() -> RandomId;
    }

    impl TRandomId for HashValue {
        fn new_id() -> RandomId {
            let mut node = [0u8; 32];
            for i in 0..4 {
                let r = rand::random::<u64>();
                node[i * 8..(i + 1) * 8].copy_from_slice(&r.to_be_bytes());
            }
            HashValue::from(&node)
        }
    }

    pub struct TestDiskScanner;

    static mut HAS_RUN: bool = false;
    impl DiskScanner for TestDiskScanner {
        fn get_cache_path_list(&self) -> Vec<(PathBuf, u64)> {
            unsafe {
                if HAS_RUN {
                    let mut list = Vec::new();
                    list.push((PathBuf::from("/test1".to_string()), 1024*1024*1024*50));
                    list.push((PathBuf::from("/test2".to_string()), 1024*1024*1024*100));
                    list.push((PathBuf::from("/test3".to_string()), 1024*1024*1024*150));
                    list.push((PathBuf::from("/test4".to_string()), 1024*1024*1024*200));
                    list.push((PathBuf::from("/test5".to_string()), 1024*1024*1024*200));
                    list
                } else {
                    HAS_RUN = true;
                    let mut list = Vec::new();
                    list.push((PathBuf::from("/test1".to_string()), 1024*1024*1024*50));
                    list.push((PathBuf::from("/test2".to_string()), 1024*1024*1024*100));
                    list.push((PathBuf::from("/test3".to_string()), 1024*1024*1024*150));
                    list.push((PathBuf::from("/test4".to_string()), 1024*1024*1024*200));
                    list.push((PathBuf::from("/test5".to_string()), 1024*1024*1024*200));
                    list
                }
            }
        }
    }
    pub struct SingleDiskChunkCacheMock {
        path: PathBuf,
        cache_id: HashValue,
        chunk_map: Mutex<HashMap<ChunkId, Box<dyn Chunk>>>,
        meta: Mutex<LocalChunkCacheMeta>,
    }

    impl TSingleDiskChunkCache for SingleDiskChunkCacheMock {
        fn new(path: PathBuf) -> Self {
            let cache_id = hash_data(path.to_string_lossy().to_string().as_bytes());
            Self {
                path,
                cache_id,
                chunk_map: Mutex::new(Default::default()),
                meta: Mutex::new(LocalChunkCacheMeta::new())
            }
        }

        fn get_cache_id(&self) -> &HashValue {
            &self.cache_id
        }

        fn get_cache_path(&self) -> &Path {
            self.path.as_path()
        }

        fn get_local_cache_meta(&self) -> BuckyResult<LocalChunkCacheMeta> {
            let meta = self.meta.lock().unwrap();
            Ok(meta.clone())
        }

        fn set_local_cache_meta(&self, meta: &LocalChunkCacheMeta) -> BuckyResult<()> {
            *self.meta.lock().unwrap() = meta.clone();
            Ok(())
        }
    }

    #[async_trait::async_trait]
    impl ChunkCache for SingleDiskChunkCacheMock {
        async fn get_chunk(&self, chunk_id: &ChunkId, _chunk_type: ChunkType) -> BuckyResult<Box<dyn Chunk>> {
            return match self.chunk_map.lock().unwrap().get(chunk_id) {
                Some(chunk) => {
                    Ok(Box::new(ChunkMock {buf: Vec::from(chunk.as_ref().deref())}))
                },
                None => {
                    Err(BuckyError::new(BuckyErrorCode::NotFound, ""))
                }
            }
        }

        async fn new_chunk(&self, _chunk_id: &ChunkId) -> BuckyResult<Box<dyn ChunkMut>> {
            todo!()
        }

        async fn delete_chunk(&self, _chunk_id: &ChunkId) -> BuckyResult<()> {
            todo!()
        }

        async fn put_chunk(&self, chunk_id: &ChunkId, chunk: &dyn Chunk) -> BuckyResult<()> {
            self.chunk_map.lock().unwrap().insert(chunk_id.clone(), Box::new(ChunkMock{buf: Vec::from(chunk.deref())}));
            Ok(())
        }

        async fn is_exist(&self, _chunk_id: &ChunkId) -> bool {
            todo!()
        }

        async fn get_chunk_meta(&self, _chunk_id: &ChunkId, _chunk_type: ChunkType) -> BuckyResult<ChunkMeta> {
            todo!()
        }
    }
    pub struct ChunkMock {
        buf: Vec<u8>
    }

    #[async_trait::async_trait]
    impl Chunk for ChunkMock {
        fn get_chunk_meta(&self) -> ChunkMeta {
            todo!()
        }

        fn get_len(&self) -> usize {
            todo!()
        }

        fn into_vec(self: Box<Self>) -> Vec<u8> {
            self.buf
        }

        async fn read(&mut self, buf: &mut [u8]) -> BuckyResult<usize> {
            // async_std::task::sleep(Duration::from_millis(10)).await;
            unsafe {
                std::ptr::copy(self.buf.as_ptr(), buf.as_mut_ptr(), self.buf.len());
            }
            Ok(self.buf.len())
        }

        async fn seek(&mut self, _pos: SeekFrom) -> BuckyResult<u64> {
            Ok(0)
        }
    }

    #[async_trait::async_trait]
    impl ChunkMut for ChunkMock {
        async fn reset(&mut self) -> BuckyResult<()> {
            todo!()
        }

        async fn write(&mut self, buf: &[u8]) -> BuckyResult<usize> {
            self.buf.append(&mut Vec::from(buf));
            Ok(self.buf.len())
        }

        async fn flush(&mut self) -> BuckyResult<()> {
            Ok(())
        }
    }

    impl Deref for ChunkMock {
        type Target = [u8];

        fn deref(&self) -> &Self::Target {
            self.buf.as_slice()
        }
    }

    impl DerefMut for ChunkMock {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.buf.as_mut_slice()
        }
    }

    #[test]
    fn test_alloc() {
        async_std::task::block_on(async move {
            let cache = LocalChunkCache::<SingleDiskChunkCacheMock, _>::new("", TestDiskScanner).await.unwrap();
            let mut chunk_list = Vec::new();
            for i in 0..1000000u32 {
                let random_id = RandomId::new_id();
                let chunk_id = ChunkId::new(&random_id, 8192*1024);
                chunk_list.push(chunk_id.clone());
                cache.put_chunk(&chunk_id, &mut ChunkMock{buf: i.to_be_bytes().to_vec()}).await.unwrap();
                let chunk = cache.get_chunk(&chunk_id, ChunkType::MemChunk).await.unwrap();
                let mut reader = ChunkRead::new(chunk);
                let mut buf = [0u8;4];
                let len = reader.read(&mut buf).await.unwrap();
                assert_eq!(len, 4);
                let tmp = u32::from_be_bytes(buf);
                assert_eq!(u32::from_be_bytes(buf), i);

                let chunk = Box::new(ChunkMock{buf: Vec::new()});
                let mut write = ChunkWrite::new(chunk);
                let len = write.write(&buf).await.unwrap();
                assert_eq!(len, 4);
            }
            {
                let list = cache.disk_cache_list.read().unwrap();
                for (item, weight) in list.iter() {
                    println!("weight {} count {}", *weight, item.chunk_map.lock().unwrap().len());
                }
            }

            cache.refresh_cache().await.unwrap();

            let mut move_count = 0;
            for chunk_id in chunk_list.iter() {
                if let Err(_) = cache.get_chunk(&chunk_id, ChunkType::MemChunk).await {
                    move_count += 1;
                }
            }

            println!("move count {}", move_count);
        })
    }
}

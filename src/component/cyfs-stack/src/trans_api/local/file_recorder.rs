use cyfs_base::*;
use cyfs_lib::*;

use async_std::prelude::*;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct AddFileRequest {
    owner: ObjectId,
}

pub(in crate::trans_api) struct FileRecorder {
    ndc: Box<dyn NamedDataCache>,
    tracker: Box<dyn TrackerCache>,
    noc: NamedObjectCacheRef,
    dec_id: ObjectId,
}

impl Clone for FileRecorder {
    fn clone(&self) -> Self {
        Self {
            ndc: self.ndc.clone(),
            tracker: self.tracker.clone(),
            noc: self.noc.clone(),
            dec_id: self.dec_id.clone(),
        }
    }
}

impl FileRecorder {
    pub fn new(
        ndc: Box<dyn NamedDataCache>,
        tracker: Box<dyn TrackerCache>,
        noc: NamedObjectCacheRef,
        dec_id: ObjectId,
    ) -> Self {
        Self {
            ndc,
            tracker,
            noc,
            dec_id,
        }
    }

    pub async fn add_file(
        &self,
        owner: &ObjectId,
        source: &Path,
        chunk_size: u32,
        dirs: Option<Vec<FileDirRef>>,
    ) -> BuckyResult<FileId> {
        let file = Self::generate_file(owner, source, chunk_size).await?;

        self.record_file(source, &file, dirs).await?;

        Ok(file.desc().file_id())
    }

    async fn generate_file(owner: &ObjectId, source: &Path, chunk_size: u32) -> BuckyResult<File> {
        // chunk_size不能太小
        if chunk_size < 1024 {
            let msg = format!("chunk size should >= 1024");
            warn!("{}", msg);
            return Err(BuckyError::new(BuckyErrorCode::InvalidParam, msg));
        }

        info!(
            "will gen file: owner={}, path={}, chunk_size={}",
            owner,
            source.display(),
            chunk_size
        );

        let (hash, len, chunk_list) = Self::create_chunk_list(source, chunk_size).await?;

        info!(
            "got file hash: path={}, hash={}, len={}",
            source.display(),
            hash,
            len
        );

        /*
        let chunk_list;
        if len <= chunk_size as u64 {

            let (hash, len) = cyfs_base::hash_file(source).await.map_err(|e| {
                let msg = format!(
                    "open file for calc hash error! file={}, {}",
                    source.display(),
                    e
                );
                error!("{}", msg);
                BuckyError::new(BuckyErrorCode::IoError, msg)
            })?;

            //整个file就是一个chunk，直接计算chunkid就好了
            let chunk_id = ChunkId::new(&hash, len as u32);
            debug!("got file chunk: id={}, file={}", chunk_id, source.display());

            chunk_list = ChunkList::ChunkInList(vec![chunk_id]);
        } else {
            //TODO: 还要支持 chunk_list太大，需要用一个chunk来保存的情况。

            chunk_list = Self::create_chunk_list(source, chunk_size).await?;
        }
        */

        let file = cyfs_base::File::new(owner.to_owned(), len, hash, chunk_list)
            .no_create_time()
            .build();
        Ok(file)
    }

    async fn create_chunk_list(
        source: &Path,
        chunk_size: u32,
    ) -> BuckyResult<(HashValue, u64, ChunkList)> {
        let mut list = Vec::new();
        let mut file = async_std::fs::File::open(source).await.map_err(|e| {
            let msg = format!(
                "open file for calc chunk list error! file={}, {}",
                source.display(),
                e
            );
            error!("{}", msg);

            BuckyError::new(BuckyErrorCode::IoError, msg)
        })?;

        use sha2::Digest;
        let mut file_sha256 = sha2::Sha256::new();
        let mut file_len = 0;
        let mut file_hash = None;
        let mut buf = Vec::with_capacity(chunk_size as usize);

        unsafe {
            buf.set_len(chunk_size as usize);
        }
        loop {
            let len = file.read(&mut buf).await?;
            if len == 0 {
                break;
            }

            let hash = cyfs_base::hash_data(&buf[0..len]);
            let chunk_id = ChunkId::new(&hash, len as u32);

            debug!(
                "got file chunk: id={}, len={}, file={}, ",
                chunk_id,
                len,
                source.display()
            );
            list.push(chunk_id);
            file_len += len;

            // 判断是不是最后一个chunk
            if len < chunk_size as usize {
                if file_len == len {
                    // 只有一个block的情况，不需要再hash一次了
                    assert!(file_hash.is_none());
                    file_hash = Some(hash);
                } else {
                    file_sha256.input(&buf[0..len]);
                }
                break;
            }

            file_sha256.input(&buf[0..len]);
        }

        let file_hash: HashValue = match file_hash {
            Some(v) => v,
            None => file_sha256.result().into(),
        };

        Ok((file_hash, file_len as u64, ChunkList::ChunkInList(list)))
    }

    async fn record_file(
        &self,
        source: &Path,
        file: &File,
        dirs: Option<Vec<FileDirRef>>,
    ) -> BuckyResult<()> {
        let file_id = file.desc().file_id();

        self.record_file_chunk_list(source, file).await?;

        // 添加到noc
        let object_raw = file.to_vec()?;
        let object = Arc::new(AnyNamedObject::Standard(StandardObject::File(file.clone())));
        let object = NONObjectInfo::new(file_id.object_id().to_owned(), object_raw, Some(object));

        let req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_dec(Some(self.dec_id.clone())),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: None,
        };

        match self.noc.put_object(&req).await {
            Ok(resp) => match resp.result {
                NamedObjectCachePutObjectResult::Accept
                | NamedObjectCachePutObjectResult::Updated => {
                    info!("insert file object to noc success success: {}", file_id);
                }
                NamedObjectCachePutObjectResult::AlreadyExists => {
                    warn!("insert object but already exists: {}", file_id);
                }
                NamedObjectCachePutObjectResult::Merged => {
                    warn!("insert file object but signs merged success: {}", file_id);
                }
            },
            Err(e) => {
                error!("insert file object to noc failed: {} {}", file_id, e);
                return Err(e);
            }
        }

        // 添加到ndc的file管理
        self.add_file_to_ndc(file, dirs).await
    }

    pub async fn add_file_to_ndc(
        &self,
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

        self.ndc.insert_file(&file_req).await.map_err(|e| {
            error!("record file to ndc error! file={}, {}", file_id, e);
            e
        })?;

        info!("record file to ndc+tracker success! file={}", file_id,);
        Ok(())
    }

    pub async fn record_file_chunk_list(&self, source: &Path, file: &File) -> BuckyResult<()> {
        let file_id = file.desc().file_id();

        let chunk_list = if let Some(body) = file.body() {
            match body.content().chunk_list() {
                ChunkList::ChunkInList(chunk_list) => {
                    if !chunk_list.is_empty() {
                        chunk_list
                    } else {
                        return Ok(());
                    }
                }
                ChunkList::ChunkInBundle(bundle) => {
                    let chunk_list = bundle.chunk_list();
                    if !chunk_list.is_empty() {
                        chunk_list
                    } else {
                        return Ok(());
                    }
                }
                ChunkList::ChunkInFile(_) => {
                    let msg = format!("ChunkInFile format not support!");
                    error!("{}", msg);
                    return Err(BuckyError::new(BuckyErrorCode::NotSupport, msg));
                }
            }
        } else {
            return Ok(());
        };

        let mut cur_pos = 0;
        for chunk_id in chunk_list {
            // 先添加到chunk索引
            let ref_obj = ChunkObjectRef {
                object_id: file_id.object_id().to_owned(),
                relation: ChunkObjectRelation::FileBody,
            };

            let req = InsertChunkRequest {
                chunk_id: chunk_id.to_owned(),
                state: ChunkState::Ready,
                ref_objects: Some(vec![ref_obj]),
                trans_sessions: None,
                flags: 0,
            };

            self.ndc.insert_chunk(&req).await.map_err(|e| {
                error!(
                    "record file chunk to ndc error! file={}, chunk={}, {}",
                    file_id, chunk_id, e
                );
                e
            })?;

            // 添加到tracker
            let pos = TrackerPostion::FileRange(PostionFileRange {
                path: source.to_str().unwrap().to_owned(),
                range_begin: cur_pos,
                range_end: cur_pos + chunk_id.len() as u64,
            });

            cur_pos += chunk_id.len() as u64;

            let req = AddTrackerPositonRequest {
                id: chunk_id.to_string(),
                direction: TrackerDirection::Store,
                pos,
                flags: 0,
            };

            if let Err(e) = self.tracker.add_position(&req).await {
                match e.code() {
                    BuckyErrorCode::AlreadyExists => {
                        // 同一个文件路径，如果部分chunk相同，会重复添加，可能触发已经存在的错误
                        warn!("record file chunk to tracker but already exists! path={}, file={}, chunk={}",
                        source.display(), file_id, chunk_id);
                    }
                    _ => {
                        error!(
                            "record file chunk to tracker error! path={}, file={}, chunk={}, {}",
                            source.display(),
                            file_id,
                            chunk_id,
                            e
                        );
                        return Err(e);
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn add_dir(
        &self,
        owner: &ObjectId,
        source: &Path,
        chunk_size: u32,
    ) -> BuckyResult<(DirId, Vec<(FileId, PathBuf)>)> {
        let mut scaner = DirScaner::new(source);
        scaner.scan_all_dir().await?;

        let mut bodys = HashMap::new();
        let mut entrys = HashMap::new();

        let file_path_list: Vec<PathBuf> = scaner.into();
        let mut file_list = Vec::new();
        let mut file_id_list = Vec::new();
        for file_path in file_path_list {
            let file = Self::generate_file(owner, &file_path, chunk_size).await?;
            let file_id = file.desc().file_id();
            let buf = file.to_vec().map_err(|e| {
                let msg = format!(
                    "encode file to buf error! path={}, file={}, {}",
                    file_path.display(),
                    file_id,
                    e
                );
                error!("{}", msg);

                BuckyError::new(e, msg)
            })?;

            bodys.insert(file_id.object_id().clone(), buf);

            let inner_file_path = file_path
                .strip_prefix(source)
                .unwrap()
                .to_string_lossy()
                .to_string();
            #[cfg(target_os = "windows")]
            let inner_file_path = inner_file_path.replace("\\", "/");
            // 内部路径不能以/开头
            let inner_file_path = inner_file_path.trim_start_matches('/').to_owned();

            entrys.insert(
                inner_file_path.clone(),
                InnerNodeInfo::new(
                    Attributes::new(0),
                    InnerNode::ObjId(file_id.object_id().clone()),
                ),
            );

            debug!(
                "got dir inner file: file_path={}, inner_path={}, file={}",
                file_path.display(),
                inner_file_path,
                file_id
            );

            file_list.push((file_path.clone(), inner_file_path, file));
            file_id_list.push((file_id, file_path));
        }

        // 生成dir
        let dir = Dir::new(
            Attributes::new(0),
            NDNObjectInfo::ObjList(NDNObjectList {
                parent_chunk: None,
                object_map: entrys,
            }),
            bodys,
        )
        .create_time(0)
        .owner(owner.to_owned())
        .build();

        let dir_id = dir.desc().dir_id();

        // 登记所有的文件
        for (file_path, inner_file_path, file) in file_list {
            let dir_ref = FileDirRef {
                dir_id: dir_id.clone(),
                inner_path: inner_file_path,
            };
            self.record_file(&file_path, &file, Some(vec![dir_ref]))
                .await?;
        }

        // 添加到noc
        let object_raw = dir.to_vec()?;
        let object = Arc::new(AnyNamedObject::Standard(StandardObject::Dir(dir)));
        let object = NONObjectInfo::new(dir_id.object_id().to_owned(), object_raw, Some(object));

        let req = NamedObjectCachePutObjectRequest {
            source: RequestSourceInfo::new_local_dec(Some(self.dec_id.clone())),
            object,
            storage_category: NamedObjectStorageCategory::Storage,
            context: None,
            last_access_rpath: None,
            access_string: None,
        };

        match self.noc.put_object(&req).await {
            Ok(resp) => match resp.result {
                NamedObjectCachePutObjectResult::Accept
                | NamedObjectCachePutObjectResult::Updated => {
                    info!("insert dir object to noc success success: {}", dir_id);
                }
                NamedObjectCachePutObjectResult::AlreadyExists => {
                    warn!("insert dir object but already exists: {}", dir_id);
                }
                NamedObjectCachePutObjectResult::Merged => {
                    warn!("insert dir object but signs merged success: {}", dir_id);
                }
            },
            Err(e) => {
                error!("insert dir object to noc failed: {} {}", dir_id, e);
                return Err(e);
            }
        }

        // TODO 需要添加目录到tracker？

        Ok((dir_id, file_id_list))
    }
}

struct DirScaner {
    roots: VecDeque<PathBuf>,
    file_list: Vec<PathBuf>,
}

impl Into<Vec<PathBuf>> for DirScaner {
    fn into(self) -> Vec<PathBuf> {
        self.file_list
    }
}

impl DirScaner {
    pub fn new(root: &Path) -> Self {
        let mut ret = Self {
            roots: VecDeque::with_capacity(64),
            file_list: Vec::with_capacity(64),
        };

        ret.roots.push_back(root.to_owned());
        ret
    }

    pub async fn scan_all_dir(&mut self) -> BuckyResult<()> {
        loop {
            match self.roots.pop_front() {
                Some(path) => {
                    self.scan_dir(&path).await?;
                }
                None => break,
            }
        }

        Ok(())
    }

    async fn scan_dir(&mut self, root: &Path) -> BuckyResult<()> {
        assert!(root.is_dir());

        let mut entries = async_std::fs::read_dir(root).await.map_err(|e| {
            error!("read dir failed! dir={}, {}", root.display(), e);
            e
        })?;

        while let Some(res) = entries.next().await {
            let entry = res.map_err(|e| {
                error!("read entry error: {}", e);
                e
            })?;

            let file_path = root.join(entry.file_name());
            if file_path.is_dir() {
                self.roots.push_back(file_path);
                continue;
            }

            if !file_path.is_file() {
                warn!("path is not file: {}", file_path.display());
                continue;
            }

            debug!("got file: {}", file_path.display());
            self.file_list.push(file_path);
        }

        Ok(())
    }
}

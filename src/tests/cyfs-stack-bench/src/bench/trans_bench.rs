use std::sync::Arc;
use async_std::io::prelude::*;
use async_trait::async_trait;
use crate::{Bench, Stat, DEVICE_DEC_ID};
use log::*;
use cyfs_base::*;
use cyfs_lib::*;
use std::convert::TryFrom;
use std::fs::{create_dir_all, remove_dir_all};
use std::path::{Path, PathBuf};

pub struct TransBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
}


pub const TRANS_ALL_IN_ONE: &str = "trans-all-in-one";
pub const TRANS_LOCAL_PUB_FILE: &str = "trans-local-pub-file";
pub const TRANS_DOWNLOAD_CHUNK: &str = "trans-download-chunk";
const LIST: [&str;2] = [
    TRANS_LOCAL_PUB_FILE,
    TRANS_DOWNLOAD_CHUNK,
];

#[async_trait]
impl Bench for TransBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await
    }

    fn name(&self) -> &str {
        "Trans Bench"
    }

    fn print_list(&self) -> Option<&[&str]> {
        Some(&LIST)
    }
}

impl TransBench {
    pub fn new(stack: SharedCyfsStack, target: Option<ObjectId>, stat: Arc<Stat>, run_times: usize) -> Box<Self> {
        Box::new(Self {
            run_times,
            stack,
            target,
            stat,
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        for i in 0..self.run_times {
            let begin = std::time::Instant::now();
            self.test_local_pub_file(i).await?;
            self.test_download_chunk(i).await?;
            self.stat.write(self.name(),TRANS_ALL_IN_ONE, begin.elapsed().as_millis() as u64);
        }

        Ok(())
    }

    async fn test_local_pub_file(&self, _i: usize) -> BuckyResult<()> {
        let begin = std::time::Instant::now();
        info!("begin test_local_pub_file...");
        let _ = self.add_random_dir(&DEVICE_DEC_ID).await;
        let (file_id, object_raw, device_id, _local_path) = self.add_random_file(&DEVICE_DEC_ID).await;
        self.download_random_file(file_id.clone(), object_raw, device_id.clone()).await;
        let (file_id, object_raw, device_id, local_path) = self.add_random_file2(&DEVICE_DEC_ID).await;
        self.download_file_impl(
            file_id.clone(),
            object_raw,
            device_id.clone(),
            &PathBuf::new(),
        )
        .await;
        self.test_get_file(file_id.clone(), device_id.clone(), &local_path).await;

        self.stat.write(self.name(),TRANS_LOCAL_PUB_FILE, begin.elapsed().as_millis() as u64);

        Ok(())
    }

    // 创建一个临时文件并覆盖
    pub async fn gen_random_file(&self, local_path: &Path) {
        if local_path.exists() {
            assert!(local_path.is_file());
            std::fs::remove_file(&local_path).unwrap();
        }

        let mut opt = async_std::fs::OpenOptions::new();
        opt.write(true).create(true).truncate(true);

        let mut f = opt.open(&local_path).await.unwrap();
        let buf_k: Vec<u8> = (0..1024).map(|_| rand::random::<u8>()).collect();
        let mut buf: Vec<u8> = Vec::with_capacity(1024 * 1024);
        for _ in 0..1024 {
            buf.extend_from_slice(&buf_k);
        }

        for _i in 0..20 {
            f.write_all(&buf).await.unwrap();
        }
        f.flush().await.unwrap();
    }

    async fn add_random_file(&self, dec_id: &ObjectId) -> (FileId, Vec<u8>, DeviceId, PathBuf) {
        let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-bench").join("trans");
        if !data_dir.exists() {
            let _ = create_dir_all(data_dir.as_path());
        }
        let local_path = data_dir.join("test-file-trans-origin");
        self.gen_random_file(&local_path).await;

        self.add_file_impl(dec_id, &local_path).await
    }

    async fn add_random_file2(&self, dec_id: &ObjectId) -> (FileId, Vec<u8>, DeviceId, PathBuf) {
        let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-bench").join("trans");
        if !data_dir.exists() {
            let _ = create_dir_all(data_dir.as_path());
        }
        let local_path = data_dir.join("test-file-trans-origin");
        self.gen_random_file(&local_path).await;

        self.add_file_impl2(dec_id, &local_path).await
    }

    fn random_string() -> String {
        hash_data(rand::random::<u64>().to_be_bytes().as_slice()).to_string()
    }

    #[async_recursion::async_recursion]
    async fn random_dir(&self, path: &Path, level: u8) {
        if level <= 2 {
            for _ in 0..2 + rand::random::<u32>() % 3 {
                let file_path = path.join(Self::random_string());
                self.gen_random_file(&file_path).await;
            }
            for _ in 0..2 + rand::random::<u32>() % 3 {
                let file_path = path.join(Self::random_string());
                let _ = create_dir_all(file_path.as_path());
                self.random_dir(file_path.as_path(), level + 1).await;
            }
        }
    }

    async fn add_random_dir(&self, dec_id: &ObjectId) -> (ObjectId, Vec<u8>, DeviceId, PathBuf) {
        let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-bench")
            .join("trans")
            .join("test_dir");
        if data_dir.exists() {
            let _ = remove_dir_all(data_dir.as_path());
        }
        let _ = create_dir_all(data_dir.as_path());

        self.random_dir(data_dir.as_path(), 1).await;

        self.add_dir_impl(dec_id, &data_dir).await
    }

    async fn add_dir_impl(
        &self,
        _dec_id: &ObjectId,
        local_path: &Path,
    ) -> (ObjectId, Vec<u8>, DeviceId, PathBuf) {
        let req = TransPublishFileOutputRequest {
            common: NDNOutputRequestCommon {
                req_path: None,
                dec_id: Some(_dec_id.clone()),
                level: Default::default(),
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            owner: Default::default(),

            // 文件的本地路径
            local_path: local_path.to_owned(),

            // chunk大小
            chunk_size: 1024 * 1024 * 4,
            // 关联的dirs
            file_id: None,
            dirs: None,
        };

        let ret = self.stack.trans().publish_file(&req).await;
        if ret.is_err() {
            error!("trans add_file error! {}", ret.unwrap_err());
            unreachable!();
        }

        let resp = ret.unwrap();
        info!("trans add file success! id={}", resp.file_id);

        let _dir_resp = self.stack
            .util()
            .build_dir_from_object_map(UtilBuildDirFromObjectMapOutputRequest {
                common: UtilOutputRequestCommon {
                    req_path: None,
                    dec_id: Some(_dec_id.clone()),
                    target: None,
                    flags: 0,
                },
                object_map_id: resp.file_id.clone(),
                dir_type: BuildDirType::Zip,
            })
            .await
            .unwrap();

        let dir_resp = self.stack
            .non_service()
            .get_object(NONGetObjectOutputRequest {
                common: NONOutputRequestCommon {
                    req_path: None,
                    source: None,
                    dec_id: None,
                    level: NONAPILevel::NOC,
                    target: None,
                    flags: 0,
                },
                object_id: _dir_resp.object_id.clone(),
                inner_path: None,
            })
            .await
            .unwrap();

        let _dir = Dir::clone_from_slice(dir_resp.object.object_raw.as_slice()).unwrap();

        (
            resp.file_id,
            Vec::new(),
            self.stack.local_device_id(),
            local_path.to_owned(),
        )
    }

    async fn add_file_impl(
        &self,
        _dec_id: &ObjectId,
        local_path: &Path,
    ) -> (FileId, Vec<u8>, DeviceId, PathBuf) {
        let req = TransPublishFileOutputRequest {
            common: NDNOutputRequestCommon {
                req_path: None,
                dec_id: Some(_dec_id.clone()),
                level: Default::default(),
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            owner: Default::default(),

            // 文件的本地路径
            local_path: local_path.to_owned(),

            // chunk大小
            chunk_size: 1024 * 1024 * 4,
            // 关联的dirs
            file_id: None,
            dirs: None,
        };

        let ret = self.stack.trans().publish_file(&req).await;
        if ret.is_err() {
            error!("trans add_file error! {}", ret.unwrap_err());
            unreachable!();
        }

        let resp = ret.unwrap();
        info!("trans add file success! id={}", resp.file_id);

        let file_id = FileId::try_from(&resp.file_id).unwrap();

        let object_raw = {
            let req = NONGetObjectRequest::new_noc(file_id.object_id().to_owned(), None);

            let resp = self.stack.non_service().get_object(req).await.unwrap();
            resp.object.object_raw
        };

        (
            file_id,
            object_raw,
            self.stack.local_device_id(),
            local_path.to_owned(),
        )
    }

    async fn add_file_impl2(
        &self,
        _dec_id: &ObjectId,
        local_path: &Path,
    ) -> (FileId, Vec<u8>, DeviceId, PathBuf) {
        let req = UtilBuildFileRequest {
            common: UtilOutputRequestCommon {
                req_path: None,
                dec_id: Some(ObjectId::default()),
                target: None,
                flags: 0,
            },
            local_path: local_path.to_path_buf(),
            owner: Default::default(),
            chunk_size: 1024 * 1024 * 4,
        };
        let ret = self.stack.util().build_file_object(req).await.unwrap();
        let file_id = ret.object_id;

        let req = TransPublishFileOutputRequest {
            common: NDNOutputRequestCommon {
                req_path: None,
                dec_id: Some(ObjectId::default()),
                level: Default::default(),
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            owner: Default::default(),

            // 文件的本地路径
            local_path: local_path.to_owned(),

            // chunk大小
            chunk_size: 1024 * 1024 * 4,
            // 关联的dirs
            file_id: Some(file_id),
            dirs: None,
        };

        let ret = self.stack.trans().publish_file(&req).await;
        if ret.is_err() {
            error!("trans add_file error! {}", ret.unwrap_err());
            unreachable!();
        }

        let resp = ret.unwrap();
        info!("trans add file success! id={}", resp.file_id);

        let file_id = FileId::try_from(&resp.file_id).unwrap();

        let object_raw = {
            let req = NONGetObjectRequest::new_noc(file_id.object_id().to_owned(), None);

            let resp = self.stack.non_service().get_object(req).await.unwrap();
            resp.object.object_raw
        };

        (
            file_id,
            object_raw,
            self.stack.local_device_id(),
            local_path.to_owned(),
        )
    }

    async fn download_random_file(&self, file_id: FileId, object_raw: Vec<u8>, device_id: DeviceId) {
        let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-bench").join("trans");
        let local_path = data_dir.join("test-file-trans");

        self.download_file_impl(file_id, object_raw, device_id, &local_path).await
    }

    async fn download_file_impl(
        &self, 
        file_id: FileId,
        object_raw: Vec<u8>,
        device_id: DeviceId,
        local_path: &Path,
    ) {
        info!(
            "will download file from device: file={}, device={}, local_path={}",
            file_id,
            device_id,
            local_path.display()
        );

        // 需要先添加到本地noc
        {
            let req = NONPutObjectOutputRequest::new_noc(file_id.object_id().to_owned(), object_raw);

            self.stack.non_service().put_object(req).await.unwrap();
        }

        // 创建下载任务
        let req = TransCreateTaskOutputRequest {
            common: NDNRequestCommon {
                req_path: None,
                dec_id: Some(ObjectId::default()),
                level: NDNAPILevel::Router,
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            object_id: file_id.object_id().to_owned(),
            local_path: local_path.to_owned(),
            device_list: vec![device_id],
            context_id: None,
            auto_start: false,
        };

        let ret = self.stack.trans().create_task(&req).await;
        if ret.is_err() {
            error!("trans create task error! {}", ret.err().unwrap());
            unreachable!();
        }

        let task_id = ret.unwrap().task_id;
        let req = TransTaskOutputRequest {
            common: NDNRequestCommon {
                req_path: None,
                dec_id: Some(ObjectId::default()),
                level: NDNAPILevel::Router,
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            task_id: task_id.clone(),
        };
        let ret = self.stack.trans().start_task(&req).await;
        if ret.is_err() {
            error!("trans start task error! {}", ret.err().unwrap());
            unreachable!()
        }
        info!("trans start task success! file={}", file_id);

        loop {
            let req = TransGetTaskStateOutputRequest {
                common: NDNOutputRequestCommon {
                    req_path: None,
                    dec_id: None,
                    level: Default::default(),
                    target: None,
                    referer_object: vec![],
                    flags: 0,
                },
                task_id: task_id.clone(),
            };

            let ret = self.stack.trans().get_task_state(&req).await;
            if ret.is_err() {
                error!("get trans task state error! {}", ret.unwrap_err());
                unreachable!();
            }

            let state = ret.unwrap().state;
            match state {
                TransTaskState::Downloading(v) => {
                    info!("trans task downloading! file_id={}, {:?}", file_id, v);
                }
                TransTaskState::Finished(_v) => {
                    info!("trans task finished! file_id={}", file_id);
                    break;
                }
                TransTaskState::Err(err) => {
                    error!(
                        "trans task canceled by err! file_id={}, err={}",
                        file_id, err
                    );
                    unreachable!();
                }
                TransTaskState::Canceled | TransTaskState::Paused | TransTaskState::Pending => {
                    unreachable!()
                }
            }

            async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        }

        let ret = self.stack
            .trans()
            .query_tasks(&TransQueryTasksOutputRequest {
                common: NDNRequestCommon {
                    req_path: None,
                    dec_id: Some(ObjectId::default()),
                    level: NDNAPILevel::Router,
                    target: None,
                    referer_object: vec![],
                    flags: 0,
                },
                context_id: None,
                task_status: Some(TransTaskStatus::Finished),
                range: Some((0, 10)),
            })
            .await;
        if ret.is_err() {
            error!("query tasks error! {}", ret.err().unwrap());
            unreachable!();
        }

        let task_list = ret.unwrap().task_list;
        assert_eq!(task_list.len(), 1);

        for task in task_list.iter() {
            let ret = self.stack
                .trans()
                .delete_task(&TransTaskOutputRequest {
                    common: NDNRequestCommon {
                        req_path: None,
                        dec_id: Some(ObjectId::default()),
                        level: NDNAPILevel::Router,
                        target: None,
                        referer_object: vec![],
                        flags: 0,
                    },
                    task_id: task.task_id.clone(),
                })
                .await;

            if ret.is_err() {
                error!("delete tasks error! {}", ret.err().unwrap());
                unreachable!();
            }
        }

        let ret = self.stack
            .trans()
            .query_tasks(&TransQueryTasksOutputRequest {
                common: NDNRequestCommon {
                    req_path: None,
                    dec_id: Some(ObjectId::default()),
                    level: NDNAPILevel::Router,
                    target: None,
                    referer_object: vec![],
                    flags: 0,
                },
                context_id: None,
                task_status: Some(TransTaskStatus::Finished),
                range: Some((0, 10)),
            })
            .await;
        if ret.is_err() {
            error!("query tasks error! {}", ret.err().unwrap());
            unreachable!();
        }

        let task_list = ret.unwrap().task_list;
        assert_eq!(task_list.len(), 0);

        info!("trans task finished success! file={}", file_id);
    }

    async fn add_chunk(&self) -> (ChunkId, Vec<u8>, DeviceId) {

        let buf: Vec<u8> = (0..3000).map(|_| rand::random::<u8>()).collect();
        let chunk_id = ChunkId::calculate(&buf).await.unwrap();

        let mut req = NDNPutDataRequest::new_with_buffer(
            NDNAPILevel::Router,
            chunk_id.object_id().to_owned(),
            buf.clone(),
        );
        // router层级，指定为none默认为所在zone的ood了，所以这里强制指定为当前协议栈
        req.common.target = Some(self.stack.local_device_id().into());

        if let Err(e) = self.stack.ndn_service().put_data(req).await {
            error!("put chunk error! {}", e);
            unreachable!();
        }

        // 立即get一次
        {
            let req = NDNGetDataRequest::new_ndc(chunk_id.object_id().to_owned(), None);

            let mut resp = self.stack.ndn_service().get_data(req).await.unwrap();
            assert_eq!(resp.length as usize, buf.len());

            let mut chunk = vec![];
            let count = resp.data.read_to_end(&mut chunk).await.unwrap();
            assert_eq!(count, resp.length as usize);
            assert_eq!(buf, chunk);
        }

        // 测试exits
        {
            //let req = NDNExistChunkRequest {
            //    chunk_id: chunk_id.to_owned(),
            //};

            //let resp = stack.ndn_service().exist_chunk(req).await.unwrap();
            //assert!(resp.exist);
        }

        let device_id = self.stack.local_device_id();
        (chunk_id, buf, device_id)
    }

    async fn download_chunk(&self, chunk_id: ChunkId, chunk: Vec<u8>, device_id: DeviceId) {
        let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-bench").join("trans");
        let local_path = data_dir.join("test-chunk");
        if local_path.exists() {
            assert!(local_path.is_file());
            std::fs::remove_file(&local_path).unwrap();
        }
    
        info!(
            "will download chunk from device: chunk={}, device={}, local_path={}",
            chunk_id,
            device_id,
            local_path.display()
        );
        
        // 创建下载任务
        let req = TransCreateTaskOutputRequest {
            common: NDNRequestCommon {
                req_path: None,
                dec_id: Some(ObjectId::default()),
                level: NDNAPILevel::Router,
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            object_id: chunk_id.object_id().to_owned(),
            local_path: local_path.clone(),
            device_list: vec![device_id.clone()],
            context_id: None,
            auto_start: false,
        };
    
        let ret = self.stack.trans().create_task(&req).await;
        if ret.is_err() {
            error!("trans start task error! {}", ret.err().unwrap());
            unreachable!();
        }
    
        let task_id = ret.unwrap().task_id;
        info!("trans create task success! file={}", chunk_id);
    
        let req = TransTaskOutputRequest {
            common: NDNRequestCommon {
                req_path: None,
                dec_id: Some(ObjectId::default()),
                level: NDNAPILevel::Router,
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            task_id: task_id.clone(),
        };
    
        let ret = self.stack.trans().start_task(&req).await;
        if ret.is_err() {
            error!("trans start task error! {}", ret.err().unwrap());
            unreachable!();
        }
    
        loop {
            let req = TransGetTaskStateOutputRequest {
                common: NDNOutputRequestCommon {
                    req_path: None,
                    dec_id: None,
                    level: Default::default(),
                    target: None,
                    referer_object: vec![],
                    flags: 0,
                },
                task_id: task_id.clone(),
            };
    
            let ret = self.stack.trans().get_task_state(&req).await;
            if ret.is_err() {
                error!("get trans task state error! {}", ret.unwrap_err());
                unreachable!();
            }
    
            let state = ret.unwrap().state;
            match state {
                TransTaskState::Downloading(v) => {
                    info!("trans task downloading! file_id={}, {:?}", chunk_id, v);
                }
                TransTaskState::Finished(_v) => {
                    info!("trans task finished! file_id={}", chunk_id);
                    break;
                }
                TransTaskState::Err(err) => {
                    error!(
                        "trans task canceled by err! file_id={}, err={}",
                        chunk_id, err
                    );
                    unreachable!();
                }
                TransTaskState::Canceled | TransTaskState::Paused | TransTaskState::Pending => {
                    unreachable!()
                }
            }
    
            async_std::task::sleep(std::time::Duration::from_secs(1)).await;
        }
    
        info!(
            "trans task finished success! chunk={}, device={}",
            chunk_id, device_id
        );
    
        // 校验chunk
        let mut opt = async_std::fs::OpenOptions::new();
        opt.read(true);
    
        let mut f = opt.open(&local_path).await.unwrap();
        let mut buf = vec![];
        let bytes = f.read_to_end(&mut buf).await.unwrap();
        assert_eq!(bytes, chunk_id.len() as usize);
        assert_eq!(buf.len(), chunk_id.len());
        assert_eq!(buf, chunk);
    }
    
    async fn test_get_chunk(&self, chunk_id: ChunkId, chunk: Vec<u8>, device_id: DeviceId) {
    
        let req = NDNGetDataRequest::new_router(
            Some(device_id.clone().into()),
            chunk_id.object_id().to_owned(),
            None,
        );
    
        info!(
            "will get chunk from device: chunk={}, device={}",
            chunk_id, device_id,
        );
    
        let mut resp = self.stack.ndn_service().get_data(req).await.unwrap();
        assert_eq!(resp.object_id, chunk_id.object_id());
    
        let mut buf = vec![];
        let size = resp.data.read_to_end(&mut buf).await.unwrap();
        assert_eq!(size, chunk_id.len());
        assert_eq!(size, chunk.len());
        assert_eq!(buf, chunk);
    
        info!(
            "get chunk from device success! file={}, len={}",
            chunk_id, size
        );
    }
    
    async fn test_get_file(&self, file_id: FileId, device_id: DeviceId, local_path: &Path) {
    
        let req = NDNGetDataRequest::new_router(
            Some(device_id.clone().into()),
            file_id.object_id().to_owned(),
            None,
        );
    
        info!(
            "will get file from device: file={}, device={}, origin_local_path={}",
            file_id,
            device_id,
            local_path.display()
        );
    
        let mut resp = self.stack.ndn_service().get_data(req).await.unwrap();
        assert_eq!(resp.object_id.obj_type_code(), ObjectTypeCode::File);
    
        assert_eq!(resp.object_id, *file_id.object_id());
    
        let mut buf = vec![];
        let size = resp.data.read_to_end(&mut buf).await.unwrap() as u64;
        assert_eq!(size, resp.length);
        let mut origin_buf = vec![];
        let bytes;
        {
            let mut f = async_std::fs::OpenOptions::new()
                .read(true)
                .open(&local_path)
                .await
                .unwrap();
            bytes = f.read_to_end(&mut origin_buf).await.unwrap() as u64;
        }
    
        assert_eq!(bytes, resp.length);
        assert_eq!(buf, origin_buf);
    
        info!(
            "get file from device success! file={}, len={}",
            file_id, bytes
        );
    }
    
    async fn test_download_chunk(&self, _i: usize) -> BuckyResult<()> {
        let begin = std::time::Instant::now();
        info!("begin test_download_chunk...");

        let ret = self.add_chunk().await;
        let ret2 = ret.clone();
        self.download_chunk(ret.0, ret.1, ret.2).await;
        self.test_get_chunk(ret2.0, ret2.1, ret2.2).await;
        self.stat.write(self.name(),TRANS_DOWNLOAD_CHUNK, begin.elapsed().as_millis() as u64);

        Ok(())
    }
}
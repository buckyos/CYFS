use async_std::io::prelude::*;
use cyfs_base::*;
use cyfs_core::*;
//use futures::{AsyncReadExt, AsyncWriteExt};
use cyfs_lib::*;
use zone_simulator::*;

use cyfs_chunk_lib::{Chunk, ChunkMeta, SharedMemChunk};
use std::convert::TryFrom;
use std::fs::{create_dir_all, remove_dir_all};
use std::ops::DerefMut;
use std::path::{Path, PathBuf};

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!("generage trans dec_id={}, people={}", dec_id, owner_id);

    dec_id
}

fn test_codec() {
    let state = TransTaskState::Err(BuckyErrorCode::Timeout);
    let v = serde_json::to_string(&state).unwrap();
    let _state2: TransTaskState = serde_json::from_str(&v).unwrap();
}

pub async fn test() {
    let dec_id = new_dec("test-trans");

    test_codec();

    test_shared_data().await;
    // 测试添加和传输一个大文件
    // test_large_file(&dec_id).await;

    let _ = add_random_dir(&dec_id).await;
    let (file_id, object_raw, device_id, _local_path) = add_random_file(&dec_id).await;
    download_random_file(file_id.clone(), object_raw, device_id.clone()).await;
    let (file_id, object_raw, device_id, local_path) = add_random_file2(&dec_id).await;
    download_file_impl(
        file_id.clone(),
        object_raw,
        device_id.clone(),
        &PathBuf::new(),
    )
    .await;
    test_get_file(file_id.clone(), device_id.clone(), &local_path).await;
    let ret = add_chunk().await;
    let ret2 = ret.clone();
    download_chunk(ret.0, ret.1, ret.2).await;
    test_get_chunk(ret2.0, ret2.1, ret2.2).await;

    test_context().await;

    info!("test all trans case success!");
}

async fn test_context() {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let ret = stack
        .trans_service()
        .get_context(&TransGetContextOutputRequest {
            common: NDNOutputRequestCommon {
                req_path: None,
                dec_id: Some(ObjectId::default()),
                level: Default::default(),
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            context_name: "test".to_string(),
        })
        .await;
    let mut context = match ret {
        Ok(context) => context,
        Err(e) => {
            if e.code() == BuckyErrorCode::NotFound {
                TransContext::new(ObjectId::default(), "test".to_string())
            } else {
                assert!(false);
                return;
            }
        }
    };
    context.set_ref_id(Some(ObjectId::default()));
    context.get_device_list_mut().push(DeviceId::default());

    stack
        .trans_service()
        .put_context(&TransPutContextOutputRequest {
            common: NDNOutputRequestCommon {
                req_path: None,
                dec_id: Some(ObjectId::default()),
                level: Default::default(),
                target: None,
                referer_object: vec![],
                flags: 0,
            },
            context,
        })
        .await
        .unwrap();
}

// 创建一个临时文件并覆盖
pub async fn gen_random_file(local_path: &Path) {
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

async fn test_large_file(dec_id: &ObjectId) {
    let local_path: PathBuf = "H:\\test.mp4".into();

    let (file_id, object_raw, device_id, _local_path) = add_file_impl(dec_id, &local_path).await;

    let local_path2: PathBuf = "H:\\test2.mp4".into();
    download_file_impl(file_id, object_raw, device_id, &local_path2).await
}

async fn add_random_file(dec_id: &ObjectId) -> (FileId, Vec<u8>, DeviceId, PathBuf) {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("trans");
    if !data_dir.exists() {
        let _ = create_dir_all(data_dir.as_path());
    }
    let local_path = data_dir.join("test-file-trans-origin");
    gen_random_file(&local_path).await;

    add_file_impl(dec_id, &local_path).await
}

async fn add_random_file2(dec_id: &ObjectId) -> (FileId, Vec<u8>, DeviceId, PathBuf) {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("trans");
    if !data_dir.exists() {
        let _ = create_dir_all(data_dir.as_path());
    }
    let local_path = data_dir.join("test-file-trans-origin");
    gen_random_file(&local_path).await;

    add_file_impl2(dec_id, &local_path).await
}

fn random_string() -> String {
    hash_data(rand::random::<u64>().to_be_bytes().as_slice()).to_string()
}

#[async_recursion::async_recursion]
async fn random_dir(path: &Path, level: u8) {
    if level <= 2 {
        for _ in 0..2 + rand::random::<u32>() % 3 {
            let file_path = path.join(random_string());
            gen_random_file(&file_path).await;
        }
        for _ in 0..2 + rand::random::<u32>() % 3 {
            let file_path = path.join(random_string());
            let _ = create_dir_all(file_path.as_path());
            random_dir(file_path.as_path(), level + 1).await;
        }
    }
}

async fn add_random_dir(dec_id: &ObjectId) -> (ObjectId, Vec<u8>, DeviceId, PathBuf) {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test")
        .join("trans")
        .join("test_dir");
    if data_dir.exists() {
        let _ = remove_dir_all(data_dir.as_path());
    }
    let _ = create_dir_all(data_dir.as_path());

    random_dir(data_dir.as_path(), 1).await;

    add_dir_impl(dec_id, &data_dir).await
}

async fn add_dir_impl(
    _dec_id: &ObjectId,
    local_path: &Path,
) -> (ObjectId, Vec<u8>, DeviceId, PathBuf) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let req = TransPublishFileOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: Some(_dec_id.clone()),
            level: Default::default(),
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        owner: USER1_DATA.get().unwrap().people_id.object_id().to_owned(),

        // 文件的本地路径
        local_path: local_path.to_owned(),

        // chunk大小
        chunk_size: 1024 * 1024 * 4,
        // 关联的dirs
        file_id: None,
        dirs: None,
    };

    let ret = stack.trans().publish_file(&req).await;
    if ret.is_err() {
        error!("trans add_file error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("trans add file success! id={}", resp.file_id);

    let _dir_resp = stack
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

    let dir_resp = stack
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
        stack.local_device_id(),
        local_path.to_owned(),
    )
}

async fn add_file_impl(
    dec_id: &ObjectId,
    local_path: &Path,
) -> (FileId, Vec<u8>, DeviceId, PathBuf) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let req = TransPublishFileOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: Some(dec_id.clone()),
            level: Default::default(),
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        owner: USER1_DATA.get().unwrap().people_id.object_id().to_owned(),

        // 文件的本地路径
        local_path: local_path.to_owned(),

        // chunk大小
        chunk_size: 1024 * 1024 * 4,
        // 关联的dirs
        file_id: None,
        dirs: None,
    };

    let ret = stack.trans().publish_file(&req).await;
    if ret.is_err() {
        error!("trans add_file error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("trans add file success! id={}", resp.file_id);

    let file_id = FileId::try_from(&resp.file_id).unwrap();

    let object_raw = {
        let req = NONGetObjectRequest::new_noc(file_id.object_id().to_owned(), None);

        let resp = stack.non_service().get_object(req).await.unwrap();
        resp.object.object_raw
    };

    (
        file_id,
        object_raw,
        stack.local_device_id(),
        local_path.to_owned(),
    )
}

async fn add_file_impl2(
    _dec_id: &ObjectId,
    local_path: &Path,
) -> (FileId, Vec<u8>, DeviceId, PathBuf) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let req = UtilBuildFileRequest {
        common: UtilOutputRequestCommon {
            req_path: None,
            dec_id: Some(ObjectId::default()),
            target: None,
            flags: 0,
        },
        local_path: local_path.to_path_buf(),
        owner: USER1_DATA.get().unwrap().people_id.object_id().to_owned(),
        chunk_size: 1024 * 1024 * 4,
    };
    let ret = stack.util().build_file_object(req).await.unwrap();
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
        owner: USER1_DATA.get().unwrap().people_id.object_id().to_owned(),

        // 文件的本地路径
        local_path: local_path.to_owned(),

        // chunk大小
        chunk_size: 1024 * 1024 * 4,
        // 关联的dirs
        file_id: Some(file_id),
        dirs: None,
    };

    let ret = stack.trans().publish_file(&req).await;
    if ret.is_err() {
        error!("trans add_file error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("trans add file success! id={}", resp.file_id);

    let file_id = FileId::try_from(&resp.file_id).unwrap();

    let object_raw = {
        let req = NONGetObjectRequest::new_noc(file_id.object_id().to_owned(), None);

        let resp = stack.non_service().get_object(req).await.unwrap();
        resp.object.object_raw
    };

    (
        file_id,
        object_raw,
        stack.local_device_id(),
        local_path.to_owned(),
    )
}

async fn download_random_file(file_id: FileId, object_raw: Vec<u8>, device_id: DeviceId) {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("trans");
    let local_path = data_dir.join("test-file-trans");

    download_file_impl(file_id, object_raw, device_id, &local_path).await
}

async fn download_file_impl(
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

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);

    // 需要先添加到本地noc
    {
        let req = NONPutObjectOutputRequest::new_noc(file_id.object_id().to_owned(), object_raw);

        stack.non_service().put_object(req).await.unwrap();
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

    let ret = stack.trans().create_task(&req).await;
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
    let ret = stack.trans().start_task(&req).await;
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

        let ret = stack.trans().get_task_state(&req).await;
        if ret.is_err() {
            error!("get trans task state error! {}", ret.unwrap_err());
            unreachable!();
        }

        let state = ret.unwrap();
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

    let ret = stack
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
        let ret = stack
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

    let ret = stack
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

async fn add_chunk() -> (ChunkId, Vec<u8>, DeviceId) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    let buf: Vec<u8> = (0..3000).map(|_| rand::random::<u8>()).collect();
    let chunk_id = ChunkId::calculate(&buf).await.unwrap();

    let mut req = NDNPutDataRequest::new_with_buffer(
        NDNAPILevel::Router,
        chunk_id.object_id().to_owned(),
        buf.clone(),
    );
    // router层级，指定为none默认为所在zone的ood了，所以这里强制指定为当前协议栈
    req.common.target = Some(stack.local_device_id().into());

    if let Err(e) = stack.ndn_service().put_data(req).await {
        error!("put chunk error! {}", e);
        unreachable!();
    }

    // 立即get一次
    {
        let req = NDNGetDataRequest::new_ndc(chunk_id.object_id().to_owned(), None);

        let mut resp = stack.ndn_service().get_data(req).await.unwrap();
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

    let device_id = stack.local_device_id();
    (chunk_id, buf, device_id)
}

async fn test_shared_data() {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    let mut shared_chunk = SharedMemChunk::new(3000, 3000, random_string().as_str()).unwrap();
    shared_chunk
        .deref_mut()
        .iter_mut()
        .for_each(|it| *it = rand::random::<u8>());
    let id = shared_chunk.calculate_id();
    if let Err(e) = stack
        .ndn_service()
        .put_shared_data(NDNPutDataRequest::new_with_buffer(
            NDNAPILevel::NDC,
            id.object_id().to_owned(),
            shared_chunk.get_chunk_meta().to_vec().unwrap(),
        ))
        .await
    {
        error!("put chunk error! {}", e);
        unreachable!();
    }

    {
        let req = NDNGetDataRequest::new_ndc(id.object_id().to_owned(), None);

        let mut resp = stack.ndn_service().get_shared_data(req).await.unwrap();
        let mut chunk_meta = vec![];
        let count = resp.data.read_to_end(&mut chunk_meta).await.unwrap();
        assert_eq!(count, chunk_meta.len());

        let shared_chunk = ChunkMeta::clone_from_slice(chunk_meta.as_slice())
            .unwrap()
            .to_chunk()
            .await
            .unwrap();

        let new_id = shared_chunk.calculate_id();
        assert_eq!(new_id, id);
    }
}
async fn download_chunk(chunk_id: ChunkId, chunk: Vec<u8>, device_id: DeviceId) {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("trans");
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

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);

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

    let ret = stack.trans().create_task(&req).await;
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

    let ret = stack.trans().start_task(&req).await;
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

        let ret = stack.trans().get_task_state(&req).await;
        if ret.is_err() {
            error!("get trans task state error! {}", ret.unwrap_err());
            unreachable!();
        }

        let state = ret.unwrap();
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

async fn test_get_chunk(chunk_id: ChunkId, chunk: Vec<u8>, device_id: DeviceId) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    let req = NDNGetDataRequest::new_router(
        Some(device_id.clone().into()),
        chunk_id.object_id().to_owned(),
        None,
    );

    info!(
        "will get chunk from device: chunk={}, device={}",
        chunk_id, device_id,
    );

    let mut resp = stack.ndn_service().get_data(req).await.unwrap();
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

async fn test_get_file(file_id: FileId, device_id: DeviceId, local_path: &Path) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

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

    let mut resp = stack.ndn_service().get_data(req).await.unwrap();
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

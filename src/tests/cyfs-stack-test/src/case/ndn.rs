use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use cyfs_util::*;
use futures::AsyncReadExt;
use zone_simulator::*;

use async_std::io::prelude::SeekExt;
use async_std::io::WriteExt;
use std::convert::TryFrom;
use std::path::Path;

pub fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!("generage ndn dec_id={}, people={}", dec_id, owner_id);

    dec_id
}

pub async fn test() {
    let dec_id = new_dec("test-ndn");

    test_range_file(&dec_id).await;

    test_chunk_in_bundle().await;
    
    // 添加目录到user1ood
    let (dir_id, file_id, inner_path, chunk_id) = add_dir(&dec_id).await;

    get_chunk(&dir_id, &file_id, &inner_path, &chunk_id, &dec_id).await;

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    get_file(&dir_id, &dec_id, &stack, true).await;

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    get_file(&dir_id, &dec_id, &stack, true).await;

    let stack = TestLoader::get_shared_stack(DeviceIndex::User2Device2);
    get_file(&dir_id, &dec_id, &stack, false).await;

    info!("test all ndn case success!");
}

pub fn gen_random_dir(dir: &Path) {
    (0..10).for_each(|i| {
        let name = format!("test{}", i);
        let dir = dir.join(&name);
        std::fs::create_dir_all(&dir).unwrap();
        (0..2).for_each(|i| {
            let name = format!("{}.log", i);
            let local_path = dir.join(&name);
            if local_path.exists() {
                std::fs::remove_file(&local_path).unwrap();
            }

            async_std::task::block_on(gen_all_random_file(&local_path));
        })
    })
}

async fn add_dir(dec_id: &ObjectId) -> (DirId, FileId, String, ChunkId) {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("root");
    gen_random_dir(&data_dir);

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let req = TransPublishFileOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: Some(dec_id.to_owned()),
            level: Default::default(),
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        owner: USER1_DATA.get().unwrap().people_id.object_id().to_owned(),

        // 文件的本地路径
        local_path: data_dir.clone(),

        // chunk大小
        chunk_size: 1024 * 1024,

        chunk_method: TransPublishChunkMethod::Track,

        access: None,

        // 关联的dirs
        file_id: None,
        dirs: None,
    };

    add_get_file_handler(&dec_id, &stack).await;

    // 事件是异步注册的，需要等待
    async_std::task::sleep(std::time::Duration::from_secs(2)).await;

    let ret = stack.trans().publish_file(req).await;
    if ret.is_err() {
        error!("trans add_dir error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("ndn add dir success! id={}", resp.file_id);

    let object_map_id = ObjectMapId::try_from(&resp.file_id).unwrap();

    let file_id_from_objectmap;
    {
        let object = {
            let mut req = NONGetObjectRequest::new_noc(
                object_map_id.object_id().to_owned(),
                Some("/test1/1.log".to_owned()),
            );
            req.common.req_path = Some("/tests/non_file".to_owned());

            let resp = stack.non_service().get_object(req).await.unwrap();
            resp.object
        };

        file_id_from_objectmap = FileId::try_from(&object.object_id).unwrap();
    }

    // convert objectmap to dir object
    let dir_id;
    {
        let req = UtilBuildDirFromObjectMapOutputRequest {
            common: UtilOutputRequestCommon::default(),
            object_map_id: object_map_id.object_id().clone(),
            dir_type: BuildDirType::Zip,
        };
        let resp = stack.util().build_dir_from_object_map(req).await.unwrap();
        dir_id = resp.object_id;
    }

    let dir_id = DirId::try_from(&dir_id).unwrap();

    let inner_path = "/test1/1.log";
    let object = {
        let mut req = NONGetObjectRequest::new_noc(
            dir_id.object_id().to_owned(),
            Some(inner_path.to_owned()),
        );
        req.common.req_path = Some("/tests/non_file".to_owned());

        let resp = stack.non_service().get_object(req).await.unwrap();
        resp.object
    };

    let file_id = FileId::try_from(&object.object_id).unwrap();
    assert_eq!(file_id, file_id_from_objectmap);

    // test query_file cases
    {
        let file = File::clone_from_slice(&object.object_raw).unwrap();
        test_query_file(dec_id, &file_id, &file).await;
    }

    // get a chunk from file for later test cases
    let chunk_list = object.object().as_file().body_expect("").content().inner_chunk_list().unwrap();
    assert!(chunk_list.len() > 0);
    info!("got file's chunk: file={}, chunk={}", file_id, chunk_list[0]);

    (dir_id, file_id, inner_path.to_owned(), chunk_list[0].clone())
}

async fn test_query_file(_dec_id: &ObjectId, id: &FileId, file: &File) {
    {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);

        let param = NDNQueryFileParam::File(id.object_id().to_owned());
        let req = NDNQueryFileOutputRequest::new_ndc(param);

        let resp = stack.ndn_service().query_file(req).await.unwrap();
        assert!(resp.list.len() == 1);
        let data = &resp.list[0];
        assert_eq!(file.desc().content().hash().to_hex_string(), *data.hash);
    }

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let ood_id = stack.local_device_id();
    {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
        let param = NDNQueryFileParam::Hash(file.desc().content().hash().to_owned());
        let req = NDNQueryFileOutputRequest::new_ndn(Some(ood_id.clone()), param);

        let resp = stack.ndn_service().query_file(req).await.unwrap();
        assert!(resp.list.len() == 1);
        let data = &resp.list[0];
        assert_eq!(file.desc().content().hash().to_hex_string(), *data.hash);
    }

    {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User2Device1);
        let param = NDNQueryFileParam::Hash(file.desc().content().hash().to_owned());
        let req = NDNQueryFileOutputRequest::new_ndn(Some(ood_id), param);

        let resp = stack.ndn_service().query_file(req).await;
        assert!(resp.is_err());
        match resp {
            Err(e) => {
                info!("query_file cross zone error: {}", e);
            }
            Ok(_) => unreachable!(),
        }
    }
}

struct OnPreRouterGetData {
    stack: String,
    reject_device: DeviceId,
}

#[async_trait::async_trait]
impl EventListenerAsyncRoutine<RouterHandlerGetDataRequest, RouterHandlerGetDataResult>
    for OnPreRouterGetData
{
    async fn call(
        &self,
        param: &RouterHandlerGetDataRequest,
    ) -> BuckyResult<RouterHandlerGetDataResult> {
        info!(
            "pre_router get_data: stack={}, request={}",
            self.stack, param.request
        );
        assert!(param.response.is_none());

        // 根据来源设备，判断是accept还是reject
        let action = if param.request.common.source.zone.device == Some(self.reject_device.clone()) {
            RouterHandlerAction::Reject
        } else {
            RouterHandlerAction::Default
        };

        let result = RouterHandlerGetDataResult {
            action,
            request: None,
            response: None,
        };

        Ok(result)
    }
}

async fn add_get_file_handler(dec_id: &ObjectId, stack: &SharedCyfsStack) {
    let reject_stack = TestLoader::get_shared_stack(DeviceIndex::User2Device2);
    let listener = OnPreRouterGetData {
        stack: stack.local_device_id().to_string(),
        reject_device: reject_stack.local_device_id(),
    };

    let filter = format!("dec_id == {} && req_path == '/shared/**'", dec_id);
    stack
        .router_handlers()
        .add_handler(
            RouterHandlerChain::PreRouter,
            "process_shared",
            0,
            Some(filter.clone()), None,
            RouterHandlerAction::Reject,
            Some(Box::new(listener)),
        )
        .unwrap();
}

async fn get_file(dir_id: &DirId, dec_id: &ObjectId, stack: &SharedCyfsStack, accpet: bool) {
    let target_stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let target = target_stack.local_device_id();

    let mut get_req = NDNGetDataOutputRequest::new_router(
        Some(target.object_id().to_owned()),
        dir_id.object_id().to_owned(),
        Some("/test1/1.log".to_owned()),
    );
    get_req.common.dec_id = Some(dec_id.to_owned());
    get_req.common.req_path = Some("/shared/xx".to_owned());

    let resp = stack.ndn_service().get_data(get_req).await;
    match resp {
        Ok(mut resp) => {
            assert!(accpet);

            info!("get file resp: {}", resp);
            let mut data = Vec::with_capacity(resp.length as usize);
            let ret = resp.data.read_to_end(&mut data).await;
            match ret {
                Ok(length) => {
                    assert_eq!(length as u64, resp.length);
                }
                Err(e) => {
                    error!("read resp data error! {}", e);
                    unreachable!();
                }
            }
        }
        Err(e) => {
            if !accpet {
                assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
            } else {
                unreachable!("{}", e);
            }
        }
    }
}

async fn get_chunk(dir_id: &DirId, file_id: &FileId, inner_path: &str, chunk_id: &ChunkId, dec_id: &ObjectId) {

    let other_dec_id = new_dec("test-ndn-other");

    // get chunk in same zone + other dec, will been allowed
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let target_stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let target = target_stack.local_device_id();

    let referer_object = NDNDataRefererObject {
        target: None,
        object_id: dir_id.object_id().to_owned(),
        inner_path: Some(inner_path.to_owned()),
    };

    let mut get_req = NDNGetDataOutputRequest::new_router(
        Some(target.object_id().to_owned()),
        chunk_id.object_id().to_owned(),
        None,
    );

    get_req.common.dec_id = Some(other_dec_id.to_owned());
    get_req.common.req_path = None;
    get_req.common.referer_object = vec![referer_object.clone()];

    stack.ndn_service().get_data(get_req).await.unwrap();


    // get chunk in diff zone + other dec, will been rejected
    let stack = TestLoader::get_shared_stack(DeviceIndex::User2Device1);
    let mut get_req = NDNGetDataOutputRequest::new_router(
        Some(target.object_id().to_owned()),
        chunk_id.object_id().to_owned(),
        None,
    );

    get_req.common.dec_id = Some(other_dec_id.to_owned());
    get_req.common.req_path = None;
    get_req.common.referer_object = vec![referer_object.clone()];

    let ret = stack.ndn_service().get_data(get_req).await;
    assert!(ret.is_err());
    
    // get chunk in diff zone, with file referer, will been rejected
    let referer_object2 = NDNDataRefererObject {
        target: None,
        object_id: file_id.object_id().to_owned(),
        inner_path: None,
    };
    let stack = TestLoader::get_shared_stack(DeviceIndex::User2Device1);
    let mut get_req = NDNGetDataOutputRequest::new_router(
        Some(target.object_id().to_owned()),
        chunk_id.object_id().to_owned(),
        None,
    );

    get_req.common.dec_id = Some(other_dec_id.to_owned());
    get_req.common.req_path = None;
    get_req.common.referer_object = vec![referer_object2.clone()];

    let ret = stack.ndn_service().get_data(get_req).await;
    assert!(ret.is_err());

    // change the file permisssions
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let access = AccessString::full_except_write();
    let mut update_req = NONUpdateObjectMetaRequest::new_router(
        None, file_id.object_id().clone(), Some(access),
    );
    update_req.common.dec_id = Some(dec_id.clone());

    stack.non_service().update_object_meta(update_req).await.unwrap();

    // get chunk in diff zone, with file referer, will success
    let referer_object2 = NDNDataRefererObject {
        target: None,
        object_id: file_id.object_id().to_owned(),
        inner_path: None,
    };
    let stack = TestLoader::get_shared_stack(DeviceIndex::User2Device1);
    let mut get_req = NDNGetDataOutputRequest::new_router(
        Some(target.object_id().to_owned()),
        chunk_id.object_id().to_owned(),
        None,
    );

    get_req.common.dec_id = Some(other_dec_id.to_owned());
    get_req.common.req_path = None;
    get_req.common.referer_object = vec![referer_object2.clone()];

    let ret = stack.ndn_service().get_data(get_req).await;
    assert!(ret.is_ok());
}

pub async fn gen_all_random_file(local_path: &Path) {
    if local_path.exists() {
        assert!(local_path.is_file());
        info!("will remove random file: {}", local_path.display());
        std::fs::remove_file(&local_path).unwrap();
    }

    info!("will gen random file: {}", local_path.display());

    let mut opt = async_std::fs::OpenOptions::new();
    opt.write(true).create(true).truncate(true);

    let mut f = opt.open(&local_path).await.unwrap();

    for _i in 0..64 {
        let buf_k: Vec<u8> = (0..1024).map(|_| rand::random::<u8>()).collect();
        f.write_all(&buf_k).await.unwrap();
    }

    f.flush().await.unwrap();
}

async fn read_file_range(local_path: &Path, range: NDNDataRange) -> Vec<u8> {
    use async_std::io::SeekFrom;

    let mut opt = async_std::fs::OpenOptions::new();
    opt.read(true).create(false);

    let mut f = opt.open(&local_path).await.unwrap();

    let start = range.start.unwrap_or(0);
    f.seek(SeekFrom::Start(start)).await.unwrap();

    let file_len = f.metadata().await.unwrap().len();
    let length = match range.length {
        Some(len) => len,
        None => file_len - start,
    };

    let mut buf: Vec<u8> = vec![0; length as usize];
    f.read_exact(&mut buf).await.unwrap();

    buf
}

async fn test_range_file(dec_id: &ObjectId) {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("root");
    std::fs::create_dir_all(&data_dir).unwrap();
    let local_path = data_dir.join("test-file-range");
    gen_all_random_file(&local_path).await;

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let ood_device_id = stack.local_device_id();
    let req = TransPublishFileOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: Some(dec_id.to_owned()),
            level: Default::default(),
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        owner: USER1_DATA.get().unwrap().people_id.object_id().to_owned(),

        // 文件的本地路径
        local_path: local_path.clone(),

        // chunk大小
        chunk_size: 1024 * 1024,

        chunk_method: TransPublishChunkMethod::Track,

        access: None,

        // 关联的dirs
        file_id: None,
        dirs: None,
    };

    let ret = stack.trans().publish_file(req).await;
    if ret.is_err() {
        error!("trans add_dir error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("ndn add range file success! id={}", resp.file_id);

    let file_id = FileId::try_from(&resp.file_id).unwrap();

    // local get
    let mut get_req = NDNGetDataOutputRequest::new_router(
        Some(ood_device_id.object_id().to_owned()),
        file_id.object_id().to_owned(),
        None,
    );
    get_req.common.dec_id = Some(dec_id.to_owned());

    let range = NDNDataRange {
        start: Some(1024),
        length: Some(1024 * 10),
    };

    // 读取文件的range
    let origin_buf = read_file_range(&local_path, range.clone()).await;

    get_req.range = Some(NDNDataRequestRange::new_data_range(vec![range]));

   
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    
    /*
    // use a not exists req_path. will been rejected by rmeta access!
    get_req.common.req_path = Some("/range/file".to_owned());

    let dec_id2 = new_dec("test-range-file-get");
    let stack = stack.fork_with_new_dec(Some(dec_id2)).await.unwrap();
    stack.wait_online(None).await.unwrap()
    let ret = stack.ndn_service().get_data(get_req.clone()).await;
    if let Err(e) = ret {
        assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
    } else {
        unreachable!();
    }
    */

    // direct use the object level access
    get_req.common.req_path = None;
    let mut resp = stack.ndn_service().get_data(get_req.clone()).await.unwrap();

    info!("get range file resp: {}", resp);
    let mut data = Vec::with_capacity(resp.length as usize);
    let ret = resp.data.read_to_end(&mut data).await;
    match ret {
        Ok(length) => {
            assert_eq!(length as u64, resp.length);
            assert_eq!(length, 1024 * 10);
            assert_eq!(data, origin_buf);
        }
        Err(e) => {
            error!("read range resp data error! {}", e);
            unreachable!();
        }
    }

    {
        let range = NDNDataRange {
            start: Some(1024 * 65),
            length: Some(1024 * 10),
        };

        get_req.range = Some(NDNDataRequestRange::new_data_range(vec![range]));

        let ret = stack.ndn_service().get_data(get_req.clone()).await;
        if let Err(e) = ret {
            info!("{}", e);
            assert_eq!(e.code(), BuckyErrorCode::RangeNotSatisfiable);
        } else {
            unreachable!();
        }
    }

    info!("test ndn file range success!")
}


pub async fn test_chunk_in_bundle() {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("root");
    std::fs::create_dir_all(&data_dir).unwrap();
    let local_path = data_dir.join("test-chunk-in-bundle");
    gen_all_random_file(&local_path).await;

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
    let req = TransPublishFileOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: None,
            level: Default::default(),
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        owner: USER1_DATA.get().unwrap().people_id.object_id().to_owned(),

        // 文件的本地路径
        local_path: local_path.clone(),

        // chunk大小
        chunk_size: 1024 * 1024,

        chunk_method: TransPublishChunkMethod::Track,

        access: None,

        // 关联的dirs
        file_id: None,
        dirs: None,
    };

    let ret = stack.trans().publish_file(req).await;
    if ret.is_err() {
        error!("trans add_dir error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("ndn add bundle file success! id={}", resp.file_id);

    // get file object
    let req = NONGetObjectRequest::new_noc(resp.file_id.clone(), None);
    let resp = stack.non_service().get_object(req).await.unwrap();
    let obj = resp.object.object();

    let file = obj.as_file();
    let chunk_list = file.body().as_ref().unwrap().content().inner_chunk_list().unwrap().to_owned();
    info!("chunk in bundle list: {:?}", chunk_list);

    // create chunk in bundle file
    let bundle = ChunkBundle::new(chunk_list.clone(), ChunkBundleHashMethod::Serial);
    let hash = bundle.calc_hash_value();
    let chunks = ChunkList::ChunkInBundle(bundle);
    let bundle_file = File::new(obj.owner().unwrap().to_owned(), 
        file.len(), hash, chunks)
            .no_create_time()
            .build();

    let bundle_file_id = bundle_file.desc().calculate_id();
    info!("bundle file id: {}", bundle_file_id);

    let req_path = "/ndn/chunk_in_bundle";

    // add chunk list to specified set to allow access
    let op_env = stack.root_state_stub(None, None).create_path_op_env().await.unwrap();
    for item in &chunk_list {
        op_env.insert(req_path, item.as_object_id()).await.unwrap();
    }
    op_env.commit().await.unwrap();

    // open the access for req_path
    {
        let meta =
            stack.root_state_meta_stub(None, None);

        let access = AccessString::full_except_write();
        let item = GlobalStatePathAccessItem {
            path: req_path.to_owned(),
            access: GlobalStatePathGroupAccess::Default(access.value()),
        };

        meta.add_access(item).await.unwrap();
    }

    let target = stack.local_device_id();
    let other_stack = TestLoader::get_shared_stack(DeviceIndex::User2OOD);

    // save bundle file to local
    let put_req = NONPutObjectRequest::new_noc(bundle_file_id.clone(), bundle_file.to_vec().unwrap());
    other_stack.non_service().put_object(put_req).await.unwrap();

    // test get from other zone, without CYFS_REQUEST_FLAG_CHUNK_LEVEL_ACL flag
    let mut ndn_req = NDNGetDataRequest::new_router(Some(target.object_id().to_owned()), bundle_file_id.clone(), None);
    ndn_req.common.req_path = Some(req_path.to_owned());
    let ret = other_stack.ndn_service().get_data(ndn_req.clone()).await;
    assert!(ret.is_err());

    // test get from other zone, with CYFS_REQUEST_FLAG_CHUNK_LEVEL_ACL flag
    ndn_req.common.flags = CYFS_REQUEST_FLAG_CHUNK_LEVEL_ACL;
    let ret = other_stack.ndn_service().get_data(ndn_req.clone()).await;
    assert!(ret.is_ok());
}
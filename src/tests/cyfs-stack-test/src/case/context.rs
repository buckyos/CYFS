use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use futures::AsyncReadExt;
use zone_simulator::*;

pub async fn test() {
    let dec_id = super::ndn::new_dec("test-ndn");
    let (target, file_id) = publish_file(&dec_id).await;
    test_ndn_get_by_context(&dec_id, target, file_id).await;
}

async fn publish_file(dec_id: &ObjectId) -> (DeviceId, FileId) {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("root");
    std::fs::create_dir_all(&data_dir).unwrap();
    let local_path = data_dir.join("test-ndn-context");
    super::ndn::gen_all_random_file(&local_path).await;

    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device1);
    let device_id = stack.local_device_id();
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

        // 关联的dirs
        file_id: None,
        dirs: None,
    };

    let ret = stack.trans().publish_file(req).await;
    if ret.is_err() {
        error!("trans add_file error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("ndn add file success! id={}", resp.file_id);

    let file_id = FileId::try_from(&resp.file_id).unwrap();
    (device_id, file_id)
}

async fn test_ndn_get_by_context(dec_id: &ObjectId, target: DeviceId, file_id: FileId) {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1Device2);

    let id1 = TransContext::gen_context_id(Some(dec_id.to_owned()), "/root");
    let id2 = TransContext::gen_context_id(Some(dec_id.to_owned()), "/root/");
    let id3 = TransContext::gen_context_id(None, "/root/");
    assert_ne!(id2, id3);

    // create context
    let mut root_context = TransContext::new(Some(dec_id.to_owned()), "/root");
    root_context.device_list_mut().push(TransContextDevice::default_stream(target.clone()));
    let context_id = root_context.desc().object_id();
    assert_eq!(context_id, id1);
    assert_eq!(context_id, id2);
    let req = TransPutContextOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: Some(dec_id.to_owned()),
            level: NDNAPILevel::Router,
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        context: root_context,
        access: None,
    };
    stack.trans().put_context(req).await.unwrap();

    // get context
    let req = TransGetContextOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: Some(dec_id.to_owned()),
            level: NDNAPILevel::Router,
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        context_path: Some("/root/".to_owned()),
        context_id: None,
    };
    let resp = stack.trans().get_context(req).await.unwrap();
    assert_eq!(resp.context.desc().object_id(), context_id);

    let req = TransGetContextOutputRequest {
        common: NDNOutputRequestCommon {
            req_path: None,
            dec_id: Some(dec_id.to_owned()),
            level: NDNAPILevel::Router,
            target: None,
            referer_object: vec![],
            flags: 0,
        },
        context_path: None,
        context_id: Some(context_id.clone()),
    };
    let resp = stack.trans().get_context(req).await.unwrap();
    assert_eq!(resp.context.desc().object_id(), context_id);

    // with error context
    let mut get_req = NDNGetDataOutputRequest::new_context(
        "/test",
        file_id.object_id().to_owned(),
        None,
    );
    get_req.common.dec_id = Some(dec_id.to_owned());

    if let Err(e) = stack.ndn_service().get_data(get_req.clone()).await {
        assert_eq!(e.code(), BuckyErrorCode::NotFound);
    } else {
        unreachable!();
    }

    // local get by path
    let mut get_req = NDNGetDataOutputRequest::new_context(
        "/root/test",
        file_id.object_id().to_owned(),
        None,
    );
    get_req.common.dec_id = Some(dec_id.to_owned());

    let mut resp = stack.ndn_service().get_data(get_req.clone()).await.unwrap();
    let mut buf = vec![];
    resp.data.read_to_end(&mut buf).await.unwrap();

    // local get by context id
    get_req.context = Some(context_id.to_string());

    let mut resp = stack.ndn_service().get_data(get_req).await.unwrap();
    let mut buf2 = vec![];
    resp.data.read_to_end(&mut buf2).await.unwrap();

    assert_eq!(buf, buf2);

    info!("test ndn get by context complete!");
}

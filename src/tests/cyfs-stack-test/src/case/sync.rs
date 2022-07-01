use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use zone_simulator::*;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

fn new_dec(name: &str) -> ObjectId {
    let owner_id = &USER1_DATA.get().unwrap().people_id;

    let dec_id = DecApp::generate_id(owner_id.object_id().to_owned(), name);

    info!("generage sync dec_id={}, people={}", dec_id, owner_id);

    dec_id
}

#[derive(Clone)]
struct Indexer {
    all: Arc<Mutex<HashMap<ObjectId, ObjectId>>>,
}

impl Indexer {
    pub fn new() -> Self {
        Self {
            all: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add(&self, root: &ObjectId, text: &ObjectId) {
        info!("index assoc: {} -> {}", root, text);
        self.all
            .lock()
            .unwrap()
            .insert(root.to_owned(), text.to_owned());
    }

    pub fn get(&self, root: &ObjectId) -> Option<ObjectId> {
        self.all.lock().unwrap().get(root).cloned()
    }
}

pub async fn test() {
    let index = Indexer::new();

    let index1 = index.clone();
    async_std::task::spawn(async move {
        let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);
        test_ood_change(&stack, index1).await;
    });

    async_std::task::spawn(async move {
        async_std::task::sleep(std::time::Duration::from_secs(5)).await;

        let stack = TestLoader::get_shared_stack(DeviceIndex::User1StandbyOOD);
        test_standby_ood_get(&stack, index).await;
    });
}

async fn add_chunk(stack: &SharedCyfsStack) -> ChunkId {
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
    } else {
        info!("put random chunk success! {}", chunk_id);
    }

    chunk_id
}

async fn add_file(stack: &SharedCyfsStack) -> FileId {
    let data_dir = cyfs_util::get_app_data_dir("cyfs-stack-test").join("sync");
    if !data_dir.exists() {
        let _ = std::fs::create_dir_all(data_dir.as_path());
    }
    let local_path = data_dir.join("test-file-sync");
    super::trans::gen_random_file(&local_path).await;

    let dec_id = new_dec("sync");
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
        error!("sync add_file error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("sync add file success! id={}", resp.file_id);

    resp.file_id.try_into().unwrap()
}

async fn test_ood_change(stack: &SharedCyfsStack, indexer: Indexer) {
    // let dec_id = new_dec("root_state1");
    let root_state = stack.root_state_stub(None, None);
    let root_info = root_state.get_current_root().await.unwrap();
    info!("current root: {:?}", root_info);

    let file_id = add_file(&stack).await;

    let mut index: usize = 0;
    loop {
        let chunk_id = add_chunk(&stack).await;
        let error_chunk_id = ChunkId::calculate_sync(&index.to_be_bytes()).unwrap();
        info!("gen error chunk: {}", error_chunk_id);

        let op_env = root_state.create_path_op_env().await.unwrap();

        let id = format!("test_text_{}", index);
        index += 1;
        let header = "test_header";
        let value = "test_value";
        let text_obj = Text::create(&id, header, value);
        let text_id = text_obj.desc().calculate_id();

        info!("new text_obj: {}, {}", id, text_id);

        op_env
            .set_with_key("/a/b", "c", &text_id, None, true)
            .await
            .unwrap();
        op_env
            .set_with_key("/a/b", "d", &text_id, None, true)
            .await
            .unwrap();
        op_env
            .set_with_key("/a/x", "e", &text_id, None, true)
            .await
            .unwrap();
        op_env
            .set_with_key("/a/z", "e", &text_id, None, true)
            .await
            .unwrap();

        op_env
            .set_with_key("/data/chunk1", "e", chunk_id.as_object_id(), None, true)
            .await
            .unwrap();
        op_env
            .set_with_path("/data/error_chunk", error_chunk_id.as_object_id(), None, true)
            .await
            .unwrap();
        op_env
            .set_with_key("/data/file1", "e", file_id.object_id(), None, true)
            .await
            .unwrap();

        let root = op_env.commit().await.unwrap();
        info!("new dec root is: {:?}, text={}", root, text_id);

        let req = NONPutObjectRequest::new_noc(text_id, text_obj.to_vec().unwrap());
        stack.non_service().put_object(req).await.unwrap();

        indexer.add(&root.root, &text_id);

        async_std::task::sleep(std::time::Duration::from_secs(60)).await;
    }
}

async fn test_standby_ood_get(stack: &SharedCyfsStack, indexer: Indexer) {
    let root_state = stack.root_state_stub(None, None);
    loop {
        let root_info = root_state.get_current_root().await.unwrap();
        info!("device current root: {:?}", root_info);

        let op_env = root_state.create_path_op_env().await.unwrap();
        let root_info = op_env.get_current_root().await.unwrap();
        info!("path bind to device current root: {:?}", root_info);

        if indexer.get(&root_info.root).is_none() {
            warn!("standby ood not sync yet!");
            async_std::task::sleep(std::time::Duration::from_secs(15)).await;
            continue;
        }

        let ret = op_env.get_by_path("/a/b/c").await.unwrap();
        let v = ret.unwrap();

        let expect = indexer.get(&root_info.root).unwrap();
        assert_eq!(expect, v);

        let ret = op_env.get_by_path("/a/z/e").await.unwrap();
        let v = ret.unwrap();

        let expect = indexer.get(&root_info.root).unwrap();
        assert_eq!(expect, v);

        info!("device will get text_object: {}", v);
        let req = NONGetObjectRequest::new_noc(v, None);
        let resp = stack.non_service().get_object(req).await.unwrap();
        let text_obj = Text::decode(&resp.object.object_raw).unwrap();
        info!("device got text_obj: {}", text_obj.id());

        async_std::task::sleep(std::time::Duration::from_secs(15)).await;
    }
}

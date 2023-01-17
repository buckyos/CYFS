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
    chunks: Arc<Mutex<Vec<ChunkId>>>,
}

impl Indexer {
    pub fn new() -> Self {
        Self {
            all: Arc::new(Mutex::new(HashMap::new())),
            chunks: Arc::new(Mutex::new(vec![])),
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

    pub fn set_chunks(&self, mut chunks: Vec<ChunkId>) {
        let mut slot = self.chunks.lock().unwrap();
        slot.clear();
        slot.append(&mut chunks);
    }

    pub fn get_chunks(&self) -> Vec<ChunkId> {
        self.chunks.lock().unwrap().clone()
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

    put_chunk(stack, chunk_id.clone(), buf).await;

    chunk_id
}

async fn put_chunk(stack: &SharedCyfsStack, chunk_id: ChunkId, buf: Vec<u8>) -> ChunkId {
    let mut req =
        NDNPutDataRequest::new_with_buffer(NDNAPILevel::Router, chunk_id.object_id(), buf.clone());
    // router层级，指定为none默认为所在zone的ood了，所以这里强制指定为当前协议栈
    req.common.target = Some(stack.local_device_id().into());

    if let Err(e) = stack.ndn_service().put_data(req).await {
        error!("put chunk error! chunk={}, {}", chunk_id, e);
        unreachable!();
    } else {
        info!("put chunk success! {}", chunk_id);
    }

    chunk_id
}

async fn put_object(stack: &SharedCyfsStack, object_id: ObjectId, object_raw: Vec<u8>) {
    let req = NONPutObjectOutputRequest::new_noc(object_id.clone(), object_raw);

    let ret = stack.non_service().put_object(req).await;
    match ret {
        Err(e) => {
            error!("put object error! object={}, {}", object_id, e);
            unreachable!();
        }
        Ok(_) => {
            info!("put object success! object={}", object_id);
        }
    }
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
        chunk_method: TransPublishChunkMethod::Track,
        access: None,
        
        // 关联的dirs
        file_id: None,
        dirs: None,
    };

    let ret = stack.trans().publish_file(req).await;
    if ret.is_err() {
        error!("sync add_file error! {}", ret.unwrap_err());
        unreachable!();
    }

    let resp = ret.unwrap();
    info!("sync add file success! id={}", resp.file_id);

    resp.file_id.try_into().unwrap()
}

async fn add_dir(stack: &SharedCyfsStack) -> (ObjectId, ChunkId) {
    let buf: Vec<u8> = (0..3000).map(|_| rand::random::<u8>()).collect();
    let chunk_id = ChunkId::calculate(&buf).await.unwrap();
    info!("new sync target chunk: {}", chunk_id);
    put_chunk(stack, chunk_id.clone(), buf).await;

    let dir_id = add_full_chunk_dir(
        stack,
        vec![("target_chunk_dir".to_owned(), chunk_id.clone())],
    )
    .await;
    let dir_id =
        add_desc_chunk_dir(stack, vec![("full_chunk_dir".to_owned(), dir_id.into())]).await;

    (dir_id, chunk_id)
}

async fn add_full_chunk_dir(stack: &SharedCyfsStack, sub: Vec<(String, ChunkId)>) -> ObjectId {
    let attr = Attributes::new(0xFFFF);
    let inner_node =
        InnerNodeInfo::new(Attributes::default(), InnerNode::ObjId(ObjectId::default()));

    let mut object_map = HashMap::new();
    object_map.insert("path1".to_owned(), inner_node);

    let buf: Vec<u8> = (0..3000).map(|_| rand::random::<u8>()).collect();
    let chunk_id = ChunkId::calculate(&buf).await.unwrap();
    let inner_node = InnerNodeInfo::new(
        Attributes::default(),
        InnerNode::ObjId(chunk_id.as_object_id().clone()),
    );
    object_map.insert("path2".to_owned(), inner_node);

    for (k, v) in sub {
        let inner_node = InnerNodeInfo::new(Attributes::default(), InnerNode::Chunk(v));
        object_map.insert(k, inner_node);
    }

    let list = NDNObjectList {
        parent_chunk: None,
        object_map,
    };

    let desc_chunk = list.to_vec().unwrap();
    let desc_chunk_id = ChunkId::calculate_sync(&desc_chunk).unwrap();

    let mut obj_map = HashMap::new();
    obj_map.insert(chunk_id.object_id(), buf);

    let body_chunk = obj_map.to_vec().unwrap();
    let body_chunk_id = ChunkId::calculate_sync(&body_chunk).unwrap();

    let builder = Dir::new_with_chunk_body(
        attr.clone(),
        NDNObjectInfo::Chunk(desc_chunk_id.clone()),
        body_chunk_id.clone(),
    );
    let dir = builder.no_create_time().update_time(0).build();
    let dir_id = dir.desc().calculate_id();

    info!("new sync full chunk dir: {}, desc={}, body={}", dir_id, desc_chunk_id, body_chunk_id);

    put_chunk(stack, desc_chunk_id, desc_chunk).await;
    put_chunk(stack, body_chunk_id, body_chunk).await;
    put_object(stack, dir_id.clone(), dir.to_vec().unwrap()).await;

    dir_id
}

async fn add_desc_chunk_dir(stack: &SharedCyfsStack, sub: Vec<(String, ObjectId)>) -> ObjectId {
    let attr = Attributes::new(0xFFFF);
    let inner_node =
        InnerNodeInfo::new(Attributes::default(), InnerNode::ObjId(ObjectId::default()));

    let mut object_map = HashMap::new();
    object_map.insert("path2".to_owned(), inner_node);
    for (k, v) in sub {
        let inner_node = InnerNodeInfo::new(Attributes::default(), InnerNode::ObjId(v));
        object_map.insert(k, inner_node);
    }

    let list = NDNObjectList {
        parent_chunk: None,
        object_map,
    };

    let desc_chunk = list.to_vec().unwrap();
    let desc_chunk_id = ChunkId::calculate_sync(&desc_chunk).unwrap();

    let obj_map = HashMap::new();
    // obj_map.insert(desc_chunk_id.object_id(), desc_chunk);

    let builder = Dir::new(
        attr.clone(),
        NDNObjectInfo::Chunk(desc_chunk_id.clone()),
        obj_map,
    );
    let dir = builder.no_create_time().update_time(0).build();
    let dir_id = dir.desc().calculate_id();

    info!("new sync desc chunk dir: {}", dir_id);

    put_chunk(stack, desc_chunk_id, desc_chunk).await;
    put_object(stack, dir_id.clone(), dir.to_vec().unwrap()).await;

    dir_id
}

async fn test_ood_change(stack: &SharedCyfsStack, indexer: Indexer) {
    // let dec_id = new_dec("root_state1");
    let root_state = stack.root_state_stub(None, None);
    let root_info = root_state.get_current_root().await.unwrap();
    info!("current root: {:?}", root_info);

    let file_id = add_file(&stack).await;

    let (dir_id, target_chunk_id) = add_dir(&stack).await;

    indexer.set_chunks(vec![target_chunk_id]);

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
            .set_with_path(
                "/data/error_chunk",
                error_chunk_id.as_object_id(),
                None,
                true,
            )
            .await
            .unwrap();
        op_env
            .set_with_key("/data/file1", "e", file_id.object_id(), None, true)
            .await
            .unwrap();

        op_env
            .set_with_path("/data/dir1", &dir_id, None, true)
            .await
            .unwrap();

        // data in object_id
        let id = ObjectIdDataBuilder::new().data(&format!("test_{}", index)).build().unwrap();
        info!("data object_id: {}", id);
        op_env
            .set_with_path("/data/object_id1", &id, None, true)
            .await
            .unwrap();
        op_env
            .set_with_path("/data/object_id2", &id, None, true)
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

        let ret = op_env.get_by_path("/data/object_id1").await.unwrap();
        let v = ret.unwrap();
        assert!(v.is_data());

        let req = RootStateAccessorGetObjectByPathOutputRequest::new("/data/object_id1");
        let ret = stack.root_state_accessor().get_object_by_path(req).await;
        assert!(ret.is_ok());
        let ret = ret.unwrap();
        assert!(ret.object.object.is_empty());
        assert!(ret.object.object.object_id.is_data());

        let object = stack.root_state_accessor_stub(None, None).get_object_by_path("/data/object_id1").await.unwrap();
        assert!(object.object.is_empty());
        assert!(object.object.object_id.is_data());
        
        info!("device will get text_object: {}", v);
        let req = NONGetObjectRequest::new_noc(v, None);
        let resp = stack.non_service().get_object(req).await.unwrap();
        let text_obj = Text::decode(&resp.object.object_raw).unwrap();
        info!("device got text_obj: {}", text_obj.id());

        let chunks = indexer.get_chunks();
        for chunk_id in chunks {
            info!("device will get chunk: {}", chunk_id);
            let req = NDNGetDataOutputRequest::new_ndc(chunk_id.object_id(), None);
            let _resp = stack.ndn_service().get_data(req).await.unwrap();
            info!("device got target chunk: {}", chunk_id);
        }

        async_std::task::sleep(std::time::Duration::from_secs(15)).await;
    }
}

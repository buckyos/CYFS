mod sqlite;
mod data_cache_manager;

#[macro_use]
extern crate log;

pub use data_cache_manager::*;

use cyfs_base::*;
use cyfs_lib::*;
use std::str::FromStr;

#[async_std::main]
async fn main() {

    cyfs_util::init_log("cyfs-ndc", Some("trace"));

    let cache = DataCacheManager::create_data_cache("test").unwrap();
    
    //test_lock().await;
    for _ in 0..100 {
        let cache1 = cache.clone();
        async_std::task::spawn(async move {
            test_file(&cache1).await;
        }).await; 
    
        let cache1 = cache.clone();
        async_std::task::spawn(async move {
            test_chunk(&cache1).await;
        }).await; 
    }

    async_std::task::sleep(std::time::Duration::from_secs(60)).await;
}

async fn monitor_task_block() {
    loop {
        async_std::task::spawn(async move {
            
        }).await;
    }
}

struct TestLocal {
    
}
async fn test_lock() {
    use std::sync::RwLock;

  
    let item = RwLock::new(None);
    let _v1 = item.read().unwrap();
    let _v2 = item.write().unwrap();
    if item.read().unwrap().is_none() {
        *item.write().unwrap() = Some(1);
    }

    use cyfs_debug::Mutex;

    async_std::task::spawn(async move {
        loop {
            async_std::task::sleep(std::time::Duration::from_secs(1)).await;
            info!("task sill alive {:?}", std::thread::current().id());
        }
    });

    for i in 1..20 {
        async_std::task::spawn(async move {
            async_std::task::sleep(std::time::Duration::from_secs(i)).await;

            info!("will block thread {:?}", std::thread::current().id());
            let item = Mutex::new(Some(1));
            let _ret = item.lock().unwrap();
            std::thread::sleep(std::time::Duration::from_secs(60));
        });
    }

    async_std::task::sleep(std::time::Duration::from_secs(60)).await;

    let item = Mutex::new(Some(1));
    let _ret = item.lock().unwrap();
    //item.lock();
    
}

async fn gen_file_info() -> (HashValue, File, FileId, Dir, DirId) {
    use std::collections::HashMap;

    let owner = ObjectId::from_str("5r4MYfFUwDfE3XbvjnHvBgWKDEmPXDWwBuxaCH9WMxNz").unwrap();
    let mut hash_value = HashValue::default();
    hash_value.as_mut_slice()[0] = 0xFF;

    let chunk_list = ChunkList::ChunkInList(Vec::new());
    let file = File::new(owner, 1024 * 1024, hash_value.clone(), chunk_list).option_create_time(None).build();
    let file_id = file.desc().file_id();

    info!("file_id={}", file_id);


    let dir = Dir::new(Attributes::new(0), NDNObjectInfo::ObjList(NDNObjectList {
        parent_chunk: None,
        object_map: HashMap::new(),
    }), HashMap::new()).create_time(0).owner(owner.clone()).build();
    let dir_id = dir.desc().dir_id();

    (hash_value, file, file_id, dir, dir_id)
}

async fn test_chunk(cache: &Box<dyn NamedDataCache>) {

    let (_hash_value, _file, file_id, _dir, dir_id) = gen_file_info().await;

    let chunk_id = ChunkId::calculate(&vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]).await.unwrap();
    let chunk_id2 = ChunkId::calculate(&vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0]).await.unwrap();
    let chunk_id3 = ChunkId::calculate(&vec![0, 1, 2, 3, 4, 5, 6, 7, 8]).await.unwrap();

    let ref_dir = ChunkObjectRef {
        object_id: file_id.clone().into(),
        relation: ChunkObjectRelation::FileBody,
    };

    let ref_dir2 = ChunkObjectRef {
        object_id: dir_id.clone().into(),
        relation: ChunkObjectRelation::FileBody,
    };

    // 测试删除逻辑
    {
        let remove_req = RemoveChunkRequest {
            chunk_id: chunk_id.clone(),
        };

        if let Err(e) = cache.remove_chunk(&remove_req).await {
            assert!(e.code() == BuckyErrorCode::NotFound);
            warn!("remove chunk but not found!");
        } else {
            info!("remove chunk success!");
        }
    }

    {
        let remove_req = RemoveChunkRequest {
            chunk_id: chunk_id2.clone(),
        };

        if let Err(e) = cache.remove_chunk(&remove_req).await {
            assert!(e.code() == BuckyErrorCode::NotFound);
            warn!("remove chunk2 but not found!");
        } else {
            info!("remove chunk2 success!");
        }
    }

    let req= InsertChunkRequest {
        chunk_id: chunk_id.clone(),
        state: ChunkState::Unknown,

        ref_objects: Some(vec![ref_dir.clone()]),
        trans_sessions: None,
        flags: 1234,
    };

    
    if let Err(e) = cache.insert_chunk(&req).await {
        error!("add chunk error: {}", e);
        unreachable!();
    } else {
        info!("add chunk success!");
    }

    let req2= InsertChunkRequest {
        chunk_id: chunk_id2.clone(),
        state: ChunkState::NotFound,

        ref_objects: Some(vec![ref_dir.clone()]),
        trans_sessions: None,
        flags: 4321,
    };

    
    if let Err(e) = cache.insert_chunk(&req2).await {
        error!("add chunk2 error: {}", e);
        unreachable!();
    } else {
        info!("add chunk2 success!");
    }

    {
        let update_req = UpdateChunkStateRequest {
            chunk_id: chunk_id.clone(),
            current_state: Some(ChunkState::NotFound),
            state: ChunkState::OnAir,
        };

        let ret = cache.update_chunk_state(&update_req).await;
        if let Err(e) = ret {
            assert!(e.code() == BuckyErrorCode::Unmatch);
            info!("update chunk state error: {}", e);
        } else {
            unreachable!();
        }
    }

    {
        let update_req = UpdateChunkStateRequest {
            chunk_id: chunk_id.clone(),
            current_state: Some(ChunkState::Unknown),
            state: ChunkState::OnAir,
        };

        let ret = cache.update_chunk_state(&update_req).await;
        if let Err(e) = ret {
            error!("update chunk state error: {}", e);
            unreachable!();
        } else {
            let state = ret.unwrap();
            assert!(state == ChunkState::Unknown);
            info!("update chunk state success!");
        }
    }

    {
        let update_req = UpdateChunkStateRequest {
            chunk_id: chunk_id.clone(),
            current_state: None,
            state: ChunkState::Ready,
        };

        let ret = cache.update_chunk_state(&update_req).await;
        if let Err(e) = ret {
            error!("update chunk state error: {}", e);
            unreachable!();
        } else {
            let state = ret.unwrap();
            assert!(state == ChunkState::OnAir);
            info!("update chunk state success!");
        }
    }

    // 测试查询
    {
        let get_req = GetChunkRequest {
            chunk_id: chunk_id.clone(),
            flags: NDC_CHUNK_REQUEST_FLAG_REF_OBJECTS,
        };

        let ret = cache.get_chunk(&get_req).await.unwrap();
        if let Some(data) = ret {
            assert!(data.state == ChunkState::Ready);
            assert!(data.flags == 1234);
           
            let mut ref_objects = data.ref_objects.unwrap();
            assert!(ref_objects.len() == 1);
            let ref_obj = ref_objects.pop().unwrap();
            assert!(ref_obj.object_id == ref_dir.object_id);
            assert!(ref_obj.relation == ref_dir.relation);

            info!("get chunk success!");
        } else {
            unreachable!("get chunk but not found");
        }
    }

    // test exists
    {
        // chunk state as follows: chunk_id: Ready, chunk_id2: NotFound, chunk_id3: not exists
        let req = ExistsChunkRequest {
            chunk_list: vec![chunk_id.clone(), chunk_id2.clone(), chunk_id3.clone()],
            states: vec![ChunkState::Unknown],
        };

        let ret = cache.exists_chunks(&req).await.unwrap();
        assert_eq!(ret, vec![false, false, false]);

        let req = ExistsChunkRequest {
            chunk_list: vec![chunk_id.clone(), chunk_id2.clone(), chunk_id3.clone()],
            states: vec![ChunkState::OnAir, ChunkState::Ready],
        };

        let ret = cache.exists_chunks(&req).await.unwrap();
        assert_eq!(ret, vec![true, false, false]);

        let req = ExistsChunkRequest {
            chunk_list: vec![chunk_id.clone(), chunk_id2.clone(), chunk_id3.clone()],
            states: vec![ChunkState::OnAir, ChunkState::Ready, ChunkState::NotFound],
        };

        let ret = cache.exists_chunks(&req).await.unwrap();
        assert_eq!(ret, vec![true, true, false]);

        info!("test chunk exists success!");
    }

    // 测试批量查询
    {
        let get_reqs = vec![GetChunkRequest{
            chunk_id: chunk_id.clone(),
            flags: 0,
        }, GetChunkRequest {
            chunk_id: chunk_id2.clone(),
            flags: NDC_CHUNK_REQUEST_FLAG_REF_OBJECTS,
        }, GetChunkRequest{
            chunk_id: chunk_id3.clone(),
            flags: 0,
        }];

        let mut ret = cache.get_chunks(&get_reqs).await.unwrap();
        assert!(ret.len() == 3);
        let r3 = ret.pop().unwrap();
        let r2 = ret.pop().unwrap();
        let r1 = ret.pop().unwrap();

        let r1 = r1.unwrap();
        assert!(r1.chunk_id == req.chunk_id);
        assert!(r1.ref_objects.is_none());
        assert!(r1.flags == req.flags);

        let r2 = r2.unwrap();
        assert!(r2.chunk_id == req2.chunk_id);
        assert!(r2.ref_objects.unwrap().len() == 1);
        assert!(r2.state == req2.state);
        assert!(r2.flags == req2.flags);

        assert!(r3.is_none());
    }

    //测试更新ref_objects
    {
        let update_req = UpdateChunkRefsRequest {
            chunk_id: chunk_id.clone(),
            add_list: vec![ref_dir2.clone()],
            remove_list: vec![ref_dir.clone()],
        };

        cache.update_chunk_ref_objects(&update_req).await.unwrap();

        let get_req = GetChunkRequest {
            chunk_id: chunk_id.clone(),
            flags: NDC_CHUNK_REQUEST_FLAG_REF_OBJECTS,
        };

        let ret = cache.get_chunk(&get_req).await.unwrap();
        let data = ret.unwrap();
        let mut ref_objects = data.ref_objects.unwrap();
        assert!(ref_objects.len() == 1);

        let ref_obj = ref_objects.pop().unwrap();
        assert!(ref_obj.object_id == ref_dir2.object_id);
        assert!(ref_obj.relation == ref_dir2.relation);

        info!("test update ref object success!");
    }

    // 测试查询ref_objects
    {
         
        let get_ref_objects_req = GetChunkRefObjectsRequest {
            chunk_id: chunk_id.clone(),
            relation: Some(ChunkObjectRelation::Unknown),
        };

        let ret = cache.get_chunk_ref_objects(&get_ref_objects_req).await.unwrap();
        assert!(ret.is_empty());

        let get_ref_objects_req = GetChunkRefObjectsRequest {
            chunk_id: chunk_id.clone(),
            relation: Some(ChunkObjectRelation::FileBody),
        };

        let mut ret = cache.get_chunk_ref_objects(&get_ref_objects_req).await.unwrap();
        assert!(ret.len() == 1);
        let ref_obj = ret.pop().unwrap();
        assert!(ref_obj.object_id == ref_dir2.object_id);
        assert!(ref_obj.relation == ref_dir2.relation);

        let get_ref_objects_req = GetChunkRefObjectsRequest {
            chunk_id: chunk_id.clone(),
            relation: None,
        };

        let mut ret = cache.get_chunk_ref_objects(&get_ref_objects_req).await.unwrap();
        assert!(ret.len() == 1);
        let ref_obj = ret.pop().unwrap();
        assert!(ref_obj.object_id == ref_dir2.object_id);
        assert!(ref_obj.relation == ref_dir2.relation);
    }
}

async fn test_file(cache: &Box<dyn NamedDataCache>) {
   
    let quick_hash_list = vec!["1111".to_owned(), "2222".to_owned()];
    
    let (hash_value, file, file_id, _dir, dir_id) = gen_file_info().await;

    let dir_refs = vec![
        FileDirRef {
            dir_id: dir_id.clone(),
            inner_path: "xxxx".to_owned(),
        },
        FileDirRef {
            dir_id: dir_id.clone(),
            inner_path: "zzzzz".to_owned(),
        }
    ];

    let req = InsertFileRequest {
        file_id: file_id.clone(),
        file: file.clone(),
        flags: 1234,
        quick_hash: Some(quick_hash_list),
        dirs: Some(dir_refs),
    };

    if let Err(e) = cache.insert_file(&req).await {
        error!("add file error: {}", e);
    } else {
        info!("add file success!");
    }

 
    {
        let hash = hash_value.to_string();
        let get_req = GetFileByHashRequest {
            hash,
            flags: 0,
        };

        let ret = cache.get_file_by_hash(&get_req).await.unwrap();
        assert!(ret.is_some());
        let data = ret.unwrap();
        info!("get file data={:?}", data);
        assert_eq!(data.file_id, file_id);
    }

    {
        let hash = hash_value.to_string();
        let get_req = GetFileByFileIdRequest {
            file_id: file_id.clone(),
            flags: NDC_FILE_REQUEST_FLAG_QUICK_HASN,
        };

        let ret = cache.get_file_by_file_id(&get_req).await.unwrap();
        assert!(ret.is_some());
        let data = ret.unwrap();
        info!("get file data={:?}", data);
        assert!(data.file_id == file_id);
        assert!(data.hash == hash);
    }

    {
        let hash = hash_value.to_string();
        let get_req = GetFileByQuickHashRequest {
            quick_hash: "1111".to_owned(),
            length: file.desc().content().len(),
            flags: NDC_FILE_REQUEST_FLAG_QUICK_HASN | NDC_FILE_REQUEST_FLAG_REF_DIRS,
        };

        let mut ret = cache.get_files_by_quick_hash(&get_req).await.unwrap();
        assert!(ret.len() == 1);
        let data = ret.pop().unwrap();
        info!("get file data={:?}", data);
        assert!(data.file_id == file_id);
        assert!(data.hash == hash);
    }

    {
        let get_req = GetFileByQuickHashRequest {
            quick_hash: "1111".to_owned(),
            length: 1234,
            flags: NDC_FILE_REQUEST_FLAG_QUICK_HASN | NDC_FILE_REQUEST_FLAG_REF_DIRS,
        };

        let ret = cache.get_files_by_quick_hash(&get_req).await.unwrap();
        assert!(ret.is_empty());
    }

    {
        let get_req = GetDirByFileRequest {
            file_id: file_id.clone(),
            flags: NDC_FILE_REQUEST_FLAG_QUICK_HASN | NDC_FILE_REQUEST_FLAG_REF_DIRS,
        };

        let ret = cache.get_dirs_by_file(&get_req).await.unwrap();
        assert!(ret.len() == 2);
    }

    {
        let remove_req = RemoveFileRequest {
            file_id,
        };

        let count = cache.remove_file(&remove_req).await.unwrap();
        assert!(count == 1);
    }
}
use cyfs_base::*;
use cyfs_lib::*;
use zone_simulator::*;

pub async fn test() {

    let (stack, file_id, owner_id) =  add_file().await;

    let item = GlobalStateObjectMetaItem {
        selector: format!("obj_type == {}", ObjectTypeCode::File.to_u16()),
        access: GlobalStatePathGroupAccess::Default(AccessString::full_except_write().value()),
        depth: None,
    };

    // remove object meta access
    {
        let meta =
            stack.root_state_meta_stub(None, None);

        meta.remove_object_meta(item.clone()).await.unwrap();
    }

    
    let ret = get_file(&file_id, &stack.local_device_id()).await;
    assert!(ret.is_err());

    // add object meta access
    {
        let meta =
            stack.root_state_meta_stub(None, None);

        let item = GlobalStateObjectMetaItem {
            selector: format!("obj_type == {}", ObjectTypeCode::File.to_u16()),
            access: GlobalStatePathGroupAccess::Default(AccessString::full_except_write().value()),
            depth: None,
        };

        meta.add_object_meta(item).await.unwrap();
    }

    let ret = get_file(&file_id, &stack.local_device_id()).await;
    assert!(ret.is_ok());

    info!("test object meta cases success!");
}

async fn get_file(file_id: &ObjectId, target: &DeviceId) -> BuckyResult<()> {
    let stack = TestLoader::get_shared_stack(DeviceIndex::User2Device1);
    let req= NONGetObjectRequest::new_router(
        Some(target.object_id().to_owned()),
        file_id.to_owned(),
        None
    );

    match stack.non_service().get_object(req).await {
        Ok(_) => Ok(()),
        Err(e) => {
            assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
            Err(e)
        }
    }
}

async fn add_file() -> (SharedCyfsStack, ObjectId, ObjectId) {
    let owner_id = &USER1_DATA.get().unwrap().people_id;
    let stack = TestLoader::get_shared_stack(DeviceIndex::User1OOD);

    let data = format!("test chunk {}", bucky_time_now()).as_bytes().to_owned();
    let chunk_id = ChunkId::calculate_sync(&data).unwrap();
    let req = NDNPutDataOutputRequest::new_with_buffer(
        NDNAPILevel::NDC,
        chunk_id.object_id(),
        data.clone(),
    );

    stack.ndn_service().put_data(req).await.unwrap();

    let hash = hash_data(&data);
    let chunk_list = ChunkList::ChunkInList(vec![chunk_id.clone()]);
    let file = File::new(owner_id.object_id().clone(), data.len() as u64, hash, chunk_list)
        .no_create_time()
        .build();

    let file_id = file.desc().calculate_id();
    info!(
        "svg file={}, chunk={}, len={}",
        file_id,
        chunk_id,
        data.len()
    );

    let req = NONPutObjectRequest::new_noc(file_id.clone(), file.to_vec().unwrap());
    stack.non_service().put_object(req).await.unwrap();

    info!(
        "put test meta file object to local noc success! file={}, owner={}",
        file_id, owner_id
    );

    (stack, file_id, owner_id.object_id().to_owned())
}

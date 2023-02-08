use super::state_manager::*;
use crate::config::StackGlobalConfig;
use crate::stack::{CyfsStackParams};
use cyfs_bdt_ext::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MemoryNOC {
    all: Arc<Mutex<HashMap<ObjectId, NamedObjectCacheObjectRawData>>>,
}

impl MemoryNOC {
    pub fn new() -> Self {
        Self {
            all: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn req_to_data(req: &NamedObjectCachePutObjectRequest) -> NamedObjectCacheObjectRawData {
        let access_string = match &req.access_string {
            Some(v) => *v,
            None => AccessString::default().value(),
        };

        let meta = NamedObjectMetaData {
            object_id: req.object.object_id.clone(),
            object_type: req.object.object().obj_type(),
            owner_id: req.object.object().owner().to_owned(),
            create_dec_id: req.source.dec.clone(),
            insert_time: 0,
            update_time: 0,
            author: req.object.object().author().to_owned(),
            dec_id: req.object.object().dec_id().to_owned(),
            object_create_time: Some(req.object.object().create_time()),
            object_update_time: req.object.object().update_time().to_owned(),
            object_expired_time: req.object.object().update_time().to_owned(),
            storage_category: req.storage_category,
            context: req.context.clone(),
            last_access_rpath: req.last_access_rpath.clone(),
            access_string,
        };

        NamedObjectCacheObjectRawData {
            object: Some(req.object.clone()),
            meta,
        }
    }
}

#[async_trait::async_trait]
impl NamedObjectCache for MemoryNOC {
    async fn put_object(
        &self,
        req: &NamedObjectCachePutObjectRequest,
    ) -> BuckyResult<NamedObjectCachePutObjectResponse> {
        let mut all = self.all.lock().unwrap();

        match all.entry(req.object.object_id.clone()) {
            Entry::Vacant(v) => {
                info!(
                    "noc first insert object: id={}, type={:?}",
                    req.object.object_id,
                    req.object.object_id.obj_type_code()
                );
                let data = Self::req_to_data(req);

                v.insert(data);
            }
            Entry::Occupied(o) => {
                let value = o.into_mut();
                info!(
                    "noc will replace object: id={}, type={:?}",
                    value.object.as_ref().unwrap().object_id,
                    value.object.as_ref().unwrap().object_id.obj_type_code()
                );
                let data = Self::req_to_data(req);
                *value = data;
            }
        }

        let resp = NamedObjectCachePutObjectResponse {
            result: NamedObjectCachePutObjectResult::Accept,
            update_time: None,
            expires_time: None,
        };

        Ok(resp)
    }

    async fn get_object_raw(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<NamedObjectCacheObjectRawData>> {
        let all = self.all.lock().unwrap();

        let ret = all.get(&req.object_id);
        if ret.is_none() {
            return Ok(None);
        }

        Ok(Some(ret.unwrap().clone()))
    }

    async fn delete_object(
        &self,
        _req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResponse> {
        unreachable!();
    }

    async fn exists_object(
        &self,
        req: &NamedObjectCacheExistsObjectRequest,
    ) -> BuckyResult<NamedObjectCacheExistsObjectResponse> {
        let all = self.all.lock().unwrap();

        let ret = if all.contains_key(&req.object_id) {
            NamedObjectCacheExistsObjectResponse {
                object: true,
                meta: true,
            }
        } else {
            NamedObjectCacheExistsObjectResponse {
                object: false,
                meta: false,
            }
        };

        Ok(ret)
    }

    async fn update_object_meta(
        &self,
        _req: &NamedObjectCacheUpdateObjectMetaRequest,
    ) -> BuckyResult<()> {
        unreachable!();
    }

    async fn check_object_access(
        &self,
        _req: &NamedObjectCacheCheckObjectAccessRequest,
    ) -> BuckyResult<Option<()>> {
        unreachable!();
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        unreachable!();
    }

    fn bind_object_meta_access_provider(
        &self,
        _object_meta_access_provider: NamedObjectCacheObjectMetaAccessProviderRef,
    ) {
        unreachable!();
    }
}

use once_cell::sync::OnceCell;
use std::str::FromStr;

pub static GLOBAL_NOC: OnceCell<NamedObjectCacheRef> = OnceCell::new();

fn init_noc() {
    // let device_id = DeviceId::from_str("5aSixgPXvhR4puWzFCHqvUXrjFWjxbq4y3thJVgZg6ty").unwrap();
    let noc = MemoryNOC::new().clone();
    if let Err(_) = GLOBAL_NOC.set(Arc::new(Box::new(noc))) {
        unreachable!();
    }
}

// 多个global_state_manager共享一个noc，用来模拟协议栈重启后的数据持久化情况
async fn create_global_state_manager() -> GlobalStateManager {
    let device_id = DeviceId::from_str("5aSixgPXvhR4puWzFCHqvUXrjFWjxbq4y3thJVgZg6ty").unwrap();
    let owner = ObjectId::from_str("5aSixgLtjoYcAFH9isc6KCqDgKfTJ8jpgASAoiRz5NLk").unwrap();
    let noc = GLOBAL_NOC.get().unwrap();
    let params = CyfsStackParams::new_default();

    let device = "0001580e4800000000661456d9a5d0503f01cbca7dc66dea0d4de188f44a3751ee66d0230000000000010a027fee89e7e40d2f9544683e480d28575794f56013a49d81b119ce3b84ff29e761000000107e8397450134fdbf16e62a576a32e35900002f4b38abe31967000140680a070a1481c0a838010a070a1481c0a864ed0a070c1481c0a838010a070c1481c0a864ed12204400000001d707019d5593b7e33a6acf5c2beb475df3feffecee8acadb8f0b741a204400000001ecdeb526690e03f1feb00d51796cc7cca0eac8a1f06915780163360100fe002f4b38abc85cb0055c4f938cd5ee2f303909cb4556d5b8033c48e69db17308d01e886e823111002645f936ed188cb9848452b9fec239f5fc8394110421ff440007b51f3e676fa2f10100ff002f4b38abe3197405863ac70379ab5cddec296d5ca292a918815e59741b22bbc8e24765d6feb25e2940c15a24979fe71ba97ffa754c405449ba64cc8a79034b579703f2fd0054b0dd";
    let mut buf = vec![];
    let bdt_params = BdtStackParams {
        device: Device::clone_from_hex(&device, &mut buf).unwrap(),
        tcp_port_mapping: vec![],
        secret: PrivateKey::generate_rsa(1024).unwrap(),
        known_sn: vec![],
        known_device: vec![],
        known_passive_pn: vec![],
        udp_sn_only: None,
    };
    let config = StackGlobalConfig::new(params, bdt_params);

    let state_manager = GlobalStateManager::load(
        GlobalStateCategory::RootState,
        &device_id,
        Some(owner),
        noc.clone(),
        config.clone(),
    )
    .await
    .unwrap();
    config.change_access_mode(GlobalStateCategory::RootState, GlobalStateAccessMode::Write);
    state_manager
}

async fn test1(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // 这里使用非托管模式env
    let op_env = root.create_op_env(None).unwrap();

    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();
    op_env
        .insert_with_key("/a/b", "test1", &x1_value)
        .await
        .unwrap();

    let current_value = op_env.get_by_key("/a/b", "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    let current_value = op_env.get_by_key("/a/b/c", "test1").await.unwrap();
    assert_eq!(current_value, None);

    // 覆盖写
    let prev_value = op_env
        .set_with_key("/a/b", "test1", &x1_value2, &Some(x1_value), false)
        .await
        .unwrap();
    assert_eq!(prev_value, Some(x1_value));

    // 提交
    let dec_root = op_env.commit().await.unwrap();
    info!("dec root changed to {}", dec_root);
    info!(
        "global root changed to {}",
        global_state_manager.get_current_root().0
    );

    // single op env, test load_with_inner_path
    let op_env = root.create_single_op_env(None).unwrap();
    op_env.load_with_inner_path(&dec_root, Some("/a/b".to_owned())).await.unwrap();

    let b_value = op_env.get_by_key("test1").await.unwrap().unwrap();
    assert_eq!(b_value, x1_value2);
}

async fn test2(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // 一组测试数据
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    // let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // 记录当前dec的root
    let dec_root = root.get_current_root();

    // 这里使用非托管模式env
    let op_env = root.create_op_env(None).unwrap();

    let current_value = op_env.get_by_key("/a/b", "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    let old = op_env
        .remove_with_key("/a/b", "test2", &None)
        .await
        .unwrap();
    assert!(old.is_none());

    // 直接获取目录b
    let b_value = op_env.get_by_key("/a", "b").await.unwrap();
    assert!(b_value.is_some());

    // 直接整体移除b目录
    let current_value = op_env.remove_with_key("/a", "b", &None).await.unwrap();
    assert_eq!(current_value, b_value);

    // b目录下面的所有值应该都不存在了
    let ret = op_env.get_by_key("/a/b", "test1").await.unwrap();
    assert!(ret.is_none());

    // 直接取消掉当前所有修改，不会对dec的root状态造成影响
    op_env.abort().unwrap();

    assert_eq!(root.get_current_root(), dec_root);

    // 再次检测状态是否正确
    let op_env = root.create_op_env(None).unwrap();

    let current_value = op_env.get_by_key("/a/b", "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    // 由于只有读取操作，所以commit不会影响dec root
    let new_root = op_env.commit().await.unwrap();
    assert_eq!(new_root, dec_root);
}

async fn test_update(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    let root_manager = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // 这里使用非托管模式env
    let op_env = root_manager.create_op_env(None).unwrap();

    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x2_value = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    let path = "/x/y";
    let path2 = "/x/y/z";

    op_env
        .insert_with_key(path, "test1", &x1_value)
        .await
        .unwrap();

    let current_value = op_env.get_by_key(path, "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    let current_value = op_env.get_by_key(path2, "test1").await.unwrap();
    assert_eq!(current_value, None);

    // update
    let root = op_env.update().await.unwrap();
    info!("dec root changed to {}", root);

    let current_value = op_env.get_by_key(path, "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    // new op_env
    let op_env2 = root_manager.create_op_env(None).unwrap();
    assert_eq!(op_env2.root(), root);

    let current_value = op_env2.get_by_key(path, "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    {
        let c_root = op_env2.update().await.unwrap();
        assert_eq!(op_env2.root(), c_root);
        assert_eq!(root, c_root);
    }

    // modify again
    let prev = op_env
        .set_with_key(path, "test1", &x2_value, &Some(x1_value), false)
        .await
        .unwrap();
    assert_eq!(Some(x1_value), prev);

    let root = op_env.update().await.unwrap();
    info!("dec root changed to {}", root);

    // 提交
    let root2 = op_env.commit().await.unwrap();
    assert_eq!(root2, root);

    info!("dec root changed to {}", root2);

    // new op_env again
    let op_env3 = root_manager.create_op_env(None).unwrap();
    assert_eq!(op_env3.root(), root2);

    let current_value = op_env3.get_by_key(path, "test1").await.unwrap();
    assert_eq!(current_value, Some(x2_value));

    info!(
        "global root changed to {}",
        global_state_manager.get_current_root().0
    );
}

async fn test_single_env(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // 首先尝试查询一下/a/b对应的object_map，用以后续校验id是否相同
    let op_env = root.create_op_env(None).unwrap();

    let b_value = op_env.get_by_key("/a/", "b").await.unwrap();
    assert!(b_value.is_some());

    // 直接使用single env操作目录b
    let single_op_env = root.create_single_op_env(None).unwrap();
    single_op_env.load_by_key("/a", "b").await.unwrap();

    let current_b = single_op_env.get_current_root().await;
    assert_eq!(current_b, b_value);

    let test1_value = single_op_env.get_by_key("test1").await.unwrap();
    assert_eq!(test1_value, Some(x1_value2));

    let prev_value = single_op_env
        .set_with_key("test1", &x1_value, &Some(x1_value2), false)
        .await
        .unwrap();
    assert_eq!(prev_value, Some(x1_value2));

    // 创建新的b，但老的仍然继续有效
    let new_b = single_op_env.commit().await.unwrap();
    info!("/a/b updated: {} -> {}", current_b.as_ref().unwrap(), new_b);
    // 校验挂在root下的对应值，没有发生改变
    {
        // /a/b没有发生改变
        let b_value = op_env.get_by_key("/a", "b").await.unwrap();
        assert!(b_value.is_some());
        assert_eq!(current_b, b_value);

        let current_value = op_env.get_by_key("/a/b", "test1").await.unwrap();
        assert_eq!(current_value, Some(x1_value2));
    }

    // 直接替换/a/b
    op_env
        .set_with_key("/a", "b", &new_b, &current_b, false)
        .await
        .unwrap();
    let new_root = op_env.commit().await.unwrap();
    info!("dec root changed to {}", new_root);
    info!(
        "global root changed to {}",
        global_state_manager.get_current_root().0
    );

    // 使用一个新的path_op_env， 校验/a/b/test1的值
    let op_env = root.create_op_env(None).unwrap();
    let current_value = op_env.get_by_key("/a/b", "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    op_env.abort().unwrap();
}

async fn test_isolate_path_env(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    let root_manager = global_state_manager
            .get_dec_root_manager(&dec_id, true)
            .await
            .unwrap();
    
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // create sub tree
    let path_env = root_manager.create_isolate_path_op_env(None).unwrap();
    path_env.get_by_path("/a/b").await.unwrap_err();

    path_env.create_new(ObjectMapSimpleContentType::Map).await.unwrap();

    path_env.insert_with_path("/a/b", &x1_value).await.unwrap();
    let ret = path_env.set_with_path("/a/b", &x1_value, &None, false).await.unwrap();
    assert_eq!(ret, Some(x1_value));
    let ret = path_env.set_with_path("/a/b", &x1_value2, &Some(x1_value), false).await.unwrap();
    assert_eq!(ret, Some(x1_value));
    let ret = path_env.get_by_path("/a/b").await.unwrap();
    assert_eq!(ret, Some(x1_value2));
    let ret = path_env.get_by_path("/a/x").await.unwrap();
    assert_eq!(ret, None);

    path_env.insert_with_path("/a/c", &x1_value).await.unwrap();

    path_env.create_new_with_path("/s", ObjectMapSimpleContentType::Set).await.unwrap();
    path_env.insert("/s", &x1_value).await.unwrap();
    path_env.insert("/s", &x1_value2).await.unwrap();

    let root = path_env.root().unwrap();
    let root2 = path_env.commit().await.unwrap();

    assert_eq!(root, root2);

    // attach to root-state and check with full path of root-state
    let op_env = root_manager.create_op_env(None).unwrap();
    op_env.insert_with_path("/i", &root).await.unwrap();

    let value = op_env.get_by_path("/i/a/b").await.unwrap();
    assert_eq!(value, Some(x1_value2));

    let value = op_env.get_by_path("/i/a/c").await.unwrap();
    assert_eq!(value, Some(x1_value));

    let value = op_env.get_by_path("/i").await.unwrap();
    assert_eq!(value, Some(root));

    let value = op_env.get_by_path("/i/a/x").await.unwrap();
    assert_eq!(value, None);

    let ret = op_env.contains("/i/s", &x1_value).await.unwrap();
    assert!(ret);

    let ret = op_env.contains("/i/s", &x1_value2).await.unwrap();
    assert!(ret);

    let ret = op_env.contains("/i/s", &root).await.unwrap();
    assert!(!ret);

    op_env.commit().await.unwrap();
}

// 托管模式的op_env
async fn test_managed(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    info!("will test managed op_env for dec={}", dec_id);

    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // 一组测试数据
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    // let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // 这里使用托管模式env
    let op_env_sid = root.create_managed_op_env(None, None).unwrap().sid();

    // 通过sid获取到对应的env才可以进行操作
    {
        let op_env = root
            .managed_envs()
            .get_path_op_env(op_env_sid, None)
            .unwrap();

        let current_value = op_env.get_by_key("/a/c", "test1").await.unwrap();
        assert!(current_value.is_none());

        let ret = op_env
            .set_with_key("/a/c", "test1", &x1_value, &None, false)
            .await;
        let e = ret.unwrap_err();
        assert_eq!(e.code(), BuckyErrorCode::NotFound);

        let ret = op_env
            .set_with_key("/a/c", "test1", &x1_value, &None, true)
            .await
            .unwrap();
        assert!(ret.is_none());

        let ret = op_env.insert_with_key("/a/c", "test1", &x1_value).await;
        let e = ret.unwrap_err();
        assert_eq!(e.code(), BuckyErrorCode::AlreadyExists);

        let current_value = op_env.get_by_key("/a/c", "test1").await.unwrap();
        assert_eq!(current_value, Some(x1_value));

        let old = op_env
            .remove_with_key("/a/c", "test1", &None)
            .await
            .unwrap();
        assert_eq!(old, current_value);

        let old = op_env.remove_with_key("/a", "c", &None).await.unwrap();
        assert!(old.is_some());
    }

    // 提交
    // 需要注意提交的时候，必须外部所有对env的引用都释放了
    let new_root = root.managed_envs().commit(op_env_sid, None).await.unwrap();
    info!("dec root udpated: dec={}, root={}", dec_id, new_root);
}

// 测试不加锁情况下的事务冲突
async fn test_conflict(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    info!("will test conflict op_env for dec={}", dec_id);

    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // 一组测试数据
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // 测试流程
    // /a/b/c/test1 = x1_value
    let op_env = root.create_op_env(None).unwrap();
    let ret = op_env
        .remove_with_key("/a/b/c", "test1", &None)
        .await
        .unwrap();
    assert!(ret.is_none());

    op_env
        .insert_with_key("/a/b/c", "test1", &x1_value)
        .await
        .unwrap();

    let dec_root = op_env.commit().await.unwrap();
    // 创建两个op_env，进行并发操作
    let op_env1 = root.create_op_env(None).unwrap();
    let op_env2 = root.create_op_env(None).unwrap();

    // op_env1进行remove操作
    let value = op_env1
        .remove_with_key("/a/b/c", "test1", &None)
        .await
        .unwrap();
    assert_eq!(value, Some(x1_value));

    // op_env2进行修改操作
    let value = op_env2
        .set_with_key("/a/b/c", "test1", &x1_value2, &None, false)
        .await
        .unwrap();
    assert_eq!(value, Some(x1_value));

    // op_env2先提交
    let new_dec_root = op_env2.commit().await.unwrap();
    info!(
        "op_env2 commit success! dec root {}->{}",
        dec_root, new_dec_root
    );

    // op_env2后提交，但因为值已经修改了，所以会提交失败
    let ret = op_env1.commit().await;
    assert!(ret.is_err());
    let e = ret.unwrap_err();
    assert_eq!(e.code(), BuckyErrorCode::Conflict);
}

// 测试不加锁情况下的事务合并
async fn test_merge(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    info!("will test merge op_env for dec={}", dec_id);

    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // 一组测试数据
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // 测试流程
    // /a/b/d/test1 = x1_value
    let op_env = root.create_op_env(None).unwrap();
    op_env
        .insert_with_key("/a/b/d", "test1", &x1_value)
        .await
        .unwrap();

    let dec_root = op_env.commit().await.unwrap();
    // 创建两个op_env，进行并发操作
    let op_env1 = root.create_op_env(None).unwrap();
    let op_env2 = root.create_op_env(None).unwrap();

    // op_env1修改test1
    let value = op_env1
        .set_with_key("/a/b/d", "test1", &x1_value2, &Some(x1_value), false)
        .await
        .unwrap();
    assert_eq!(value, Some(x1_value));

    // op_env2增加一个test2，不影响test1
    let value = op_env2
        .set_with_key("/a/b/d", "test2", &x1_value2, &None, true)
        .await
        .unwrap();
    assert!(value.is_none());

    // op_env1提交
    let new_dec_root = op_env1.commit().await.unwrap();
    info!(
        "op_env1 commit success! dec root {}->{}",
        dec_root, new_dec_root
    );

    assert_eq!(new_dec_root, root.get_current_root());

    // op_env2再提交
    let new_dec_root2 = op_env2.commit().await.unwrap();
    info!(
        "op_env2 commit success! dec root {}->{}",
        new_dec_root, new_dec_root2
    );

    assert_eq!(new_dec_root2, root.get_current_root());
}

async fn test_path_lock(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    info!("will test path lock op_env for dec={}", dec_id);

    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // 一组测试数据
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let _x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // 测试流程
    // /a/b/d/test1 = x1_value
    let op_env = root.create_op_env(None).unwrap();
    let op_env2 = root.create_op_env(None).unwrap();

    op_env
        .lock_path(vec!["/a/b".to_owned()], 0, true)
        .await
        .unwrap();

    // 所有操作对锁的依赖都不是强制的，属于君子协议
    op_env
        .insert_with_key("/a/b/e", "test1", &x1_value)
        .await
        .unwrap();

    // 相同目录、子目录、父目录会加锁失败
    op_env2
        .lock_path(vec!["/a".to_owned()], 0, true)
        .await
        .unwrap_err();
    op_env2
        .lock_path(vec!["/a/b".to_owned()], 0, true)
        .await
        .unwrap_err();
    op_env2
        .lock_path(vec!["/a/b/c".to_owned()], 0, true)
        .await
        .unwrap_err();

    // 其余可以加锁成功
    op_env2
        .lock_path(vec!["/a/c".to_owned()], 0, true)
        .await
        .unwrap();
    op_env2
        .lock_path(vec!["/a/d".to_owned()], 0, true)
        .await
        .unwrap();
    op_env2
        .lock_path(vec!["/d/a".to_owned()], 0, true)
        .await
        .unwrap();

    // 部分失败则失败
    op_env2
        .lock_path(vec!["/a/c".to_owned(), "/a/b".to_owned()], 0, true)
        .await
        .unwrap_err();

    // op_env在commit或者abort后，会自动释放所有lock
    let _dec_root = op_env.commit().await.unwrap();

    op_env2
        .lock_path(vec!["/a".to_owned(), "/a/b".to_owned()], 0, true)
        .await
        .unwrap();
}

async fn test_remove_panic(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    let env = root.create_op_env(None).unwrap();

    let header = "cyfs system";
    let value = "xxxxx";
    let obj = Text::create("cyfs", header, value);

    let object_id = obj.desc().object_id();
    let ret = env
        .insert_with_key("/test/", "test_panic", &object_id)
        .await
        .unwrap();
    info!("insert {:?}", ret);
    let root1 = env.commit().await.unwrap();
    info!("new dec root is: {}", root1);

    let env2 = root.create_op_env(None).unwrap();
    env2.remove_with_key("/test/", "test_panic", &Some(object_id))
        .await
        .unwrap();
    // env2.remove_with_path("/test/panic",  Some(object_id)).await.unwrap();
    let root2 = env2.commit().await.unwrap();
    info!("new dec root is: {}", root2);
}

async fn test() {
    init_noc();

    let global_state_manager = create_global_state_manager().await;

    let owner = ObjectId::from_str("5aSixgLtjoYcAFH9isc6KCqDgKfTJ8jpgASAoiRz5NLk").unwrap();
    let dec_id = DecApp::generate_id(owner, "test1");

    test_remove_panic(&global_state_manager, &dec_id).await;
    test1(&global_state_manager, &dec_id).await;
    test_single_env(&global_state_manager, &dec_id).await;

    let global_state_manager2 = create_global_state_manager().await;
    test2(&global_state_manager2, &dec_id).await;

    test_isolate_path_env(&global_state_manager, &dec_id).await;

    test_update(&global_state_manager2, &dec_id).await;

    test_managed(&global_state_manager2, &dec_id).await;

    test_conflict(&global_state_manager2, &dec_id).await;

    test_merge(&global_state_manager2, &dec_id).await;

    test_path_lock(&global_state_manager2, &dec_id).await;
}

#[test]
fn main() {
    cyfs_util::init_log("test-global-state-manager", Some("debug"));
    async_std::task::block_on(async move {
        test().await;
    });
}

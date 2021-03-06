use super::{state_manager::*};
use crate::config::StackGlobalConfig;
use crate::stack::CyfsStackParams;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;

use std::collections::{hash_map::Entry, HashMap};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct MemoryNOC {
    all: Arc<Mutex<HashMap<ObjectId, ObjectCacheData>>>,
}

impl MemoryNOC {
    pub fn new() -> Self {
        Self {
            all: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn req_to_data(obj_info: &NamedObjectCacheInsertObjectRequest) -> ObjectCacheData {
        let now = bucky_time_now();

        let mut data = ObjectCacheData {
            protocol: obj_info.protocol.clone(),
            source: obj_info.source.clone(),
            object_id: obj_info.object_id,
            dec_id: obj_info.dec_id,
            object_raw: Some(obj_info.object_raw.clone()),
            object: None,
            flags: obj_info.flags,
            create_time: now,
            update_time: now,
            insert_time: now,
            rank: 10,
        };

        data.rebuild_object().unwrap();

        data
    }
}

#[async_trait::async_trait]
impl NamedObjectCache for MemoryNOC {
    async fn insert_object(
        &self,
        obj_info: &NamedObjectCacheInsertObjectRequest,
    ) -> BuckyResult<NamedObjectCacheInsertResponse> {
        let mut all = self.all.lock().unwrap();

        match all.entry(obj_info.object_id.clone()) {
            Entry::Vacant(v) => {
                info!(
                    "noc first insert object: id={}, type={:?}",
                    obj_info.object_id,
                    obj_info.object_id.obj_type_code()
                );
                let data = Self::req_to_data(obj_info);

                v.insert(data);
            }
            Entry::Occupied(o) => {
                let value = o.into_mut();
                info!(
                    "noc will replace object: id={}, type={:?}",
                    obj_info.object_id,
                    obj_info.object_id.obj_type_code()
                );
                let data = Self::req_to_data(obj_info);
                *value = data;
            }
        }

        let resp = NamedObjectCacheInsertResponse {
            result: NamedObjectCacheInsertResult::Accept,
            object_update_time: None,
            object_expires_time: None,
        };

        Ok(resp)
    }

    async fn get_object(
        &self,
        req: &NamedObjectCacheGetObjectRequest,
    ) -> BuckyResult<Option<ObjectCacheData>> {
        let all = self.all.lock().unwrap();

        let ret = all.get(&req.object_id);
        if ret.is_none() {
            return Ok(None);
        }

        Ok(Some(ret.unwrap().clone()))
    }

    async fn select_object(
        &self,
        _req: &NamedObjectCacheSelectObjectRequest,
    ) -> BuckyResult<Vec<ObjectCacheData>> {
        unreachable!();
    }

    async fn delete_object(
        &self,
        _req: &NamedObjectCacheDeleteObjectRequest,
    ) -> BuckyResult<NamedObjectCacheDeleteObjectResult> {
        unreachable!();
    }

    async fn stat(&self) -> BuckyResult<NamedObjectCacheStat> {
        unreachable!();
    }

    fn sync_server(&self) -> Option<Box<dyn NamedObjectCacheSyncServer>> {
        unreachable!();
    }

    fn sync_client(&self) -> Option<Box<dyn NamedObjectCacheSyncClient>> {
        unreachable!();
    }

    fn clone_noc(&self) -> Box<dyn NamedObjectCache> {
        Box::new(Clone::clone(&self as &MemoryNOC)) as Box<dyn NamedObjectCache>
    }
}

use once_cell::sync::OnceCell;
use std::str::FromStr;

pub static GLOBAL_NOC: OnceCell<Box<dyn NamedObjectCache>> = OnceCell::new();

fn init_noc() {
    let device_id = DeviceId::from_str("5aSixgPXvhR4puWzFCHqvUXrjFWjxbq4y3thJVgZg6ty").unwrap();
    let noc = MemoryNOC::new().clone_noc();
    if let Err(_) = GLOBAL_NOC.set(noc) {
        unreachable!();
    }
}

// ??????global_state_manager????????????noc?????????????????????????????????????????????????????????
async fn create_global_state_manager() -> GlobalStateManager {
    let device_id = DeviceId::from_str("5aSixgPXvhR4puWzFCHqvUXrjFWjxbq4y3thJVgZg6ty").unwrap();
    let owner = ObjectId::from_str("5aSixgLtjoYcAFH9isc6KCqDgKfTJ8jpgASAoiRz5NLk").unwrap();
    let noc = GLOBAL_NOC.get().unwrap();
    let params = CyfsStackParams::new_default();
    let config = StackGlobalConfig::new(params);

    let state_manager = GlobalStateManager::load(
        GlobalStateCategory::RootState,
        &device_id,
        Some(owner),
        noc.clone_noc(),
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

    // ???????????????????????????env
    let op_env = root.create_op_env().await.unwrap();

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

    // ?????????
    let prev_value = op_env
        .set_with_key("/a/b", "test1", &x1_value2, &Some(x1_value), false)
        .await
        .unwrap();
    assert_eq!(prev_value, Some(x1_value));

    // ??????
    let root = op_env.commit().await.unwrap();
    info!("dec root changed to {}", root);
    info!(
        "global root changed to {}",
        global_state_manager.get_current_root().0
    );
}

async fn test2(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // ??????????????????
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // ????????????dec???root
    let dec_root = root.get_current_root();

    // ???????????????????????????env
    let op_env = root.create_op_env().await.unwrap();

    let current_value = op_env.get_by_key("/a/b", "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    let old = op_env
        .remove_with_key("/a/b", "test2", &None)
        .await
        .unwrap();
    assert!(old.is_none());

    // ??????????????????b
    let b_value = op_env.get_by_key("/a", "b").await.unwrap();
    assert!(b_value.is_some());

    // ??????????????????b??????
    let current_value = op_env.remove_with_key("/a", "b", &None).await.unwrap();
    assert_eq!(current_value, b_value);

    // b?????????????????????????????????????????????
    let ret = op_env.get_by_key("/a/b", "test1").await.unwrap();
    assert!(ret.is_none());

    // ?????????????????????????????????????????????dec???root??????????????????
    op_env.abort().unwrap();

    assert_eq!(root.get_current_root(), dec_root);

    // ??????????????????????????????
    let op_env = root.create_op_env().await.unwrap();

    let current_value = op_env.get_by_key("/a/b", "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    // ?????????????????????????????????commit????????????dec root
    let new_root = op_env.commit().await.unwrap();
    assert_eq!(new_root, dec_root);
}

async fn test_update(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    let root_manager = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // ???????????????????????????env
    let op_env = root_manager.create_op_env().await.unwrap();

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
    let op_env2 = root_manager.create_op_env().await.unwrap();
    assert_eq!(op_env2.root(),  root);

    let current_value = op_env2.get_by_key(path, "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    {
        let c_root = op_env2.update().await.unwrap();
        assert_eq!(op_env2.root(),  c_root);
        assert_eq!(root,  c_root);
    }
    
    // modify again
    let prev = op_env.set_with_key(path, "test1", &x2_value, &Some(x1_value), false).await.unwrap();
    assert_eq!(Some(x1_value), prev);

    let root = op_env.update().await.unwrap();
    info!("dec root changed to {}", root);

    // ??????
    let root2 = op_env.commit().await.unwrap();
    assert_eq!(root2, root);

    info!("dec root changed to {}", root2);

    // new op_env again
    let op_env3 = root_manager.create_op_env().await.unwrap();
    assert_eq!(op_env3.root(),  root2);

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

    // ????????????????????????/a/b?????????object_map?????????????????????id????????????
    let op_env = root.create_op_env().await.unwrap();

    let b_value = op_env.get_by_key("/a/", "b").await.unwrap();
    assert!(b_value.is_some());

    // ????????????single env????????????b
    let single_op_env = root.create_single_op_env().unwrap();
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

    // ????????????b??????????????????????????????
    let new_b = single_op_env.commit().await.unwrap();
    info!("/a/b updated: {} -> {}", current_b.as_ref().unwrap(), new_b);
    // ????????????root????????????????????????????????????
    {
        // /a/b??????????????????
        let b_value = op_env.get_by_key("/a", "b").await.unwrap();
        assert!(b_value.is_some());
        assert_eq!(current_b, b_value);

        let current_value = op_env.get_by_key("/a/b", "test1").await.unwrap();
        assert_eq!(current_value, Some(x1_value2));
    }

    // ????????????/a/b
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

    // ??????????????????path_op_env??? ??????/a/b/test1??????
    let op_env = root.create_op_env().await.unwrap();
    let current_value = op_env.get_by_key("/a/b", "test1").await.unwrap();
    assert_eq!(current_value, Some(x1_value));

    op_env.abort().unwrap();
}

// ???????????????op_env
async fn test_managed(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    info!("will test managed op_env for dec={}", dec_id);

    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // ??????????????????
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // ????????????????????????env
    let op_env_sid = root.create_managed_op_env().await.unwrap().sid();

    // ??????sid??????????????????env?????????????????????
    {
        let op_env = root.managed_envs().get_path_op_env(op_env_sid).unwrap();

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

    // ??????
    // ???????????????????????????????????????????????????env?????????????????????
    let new_root = root.managed_envs().commit(op_env_sid).await.unwrap();
    info!("dec root udpated: dec={}, root={}", dec_id, new_root);
}

// ???????????????????????????????????????
async fn test_conflict(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    info!("will test conflict op_env for dec={}", dec_id);

    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // ??????????????????
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // ????????????
    // /a/b/c/test1 = x1_value
    let op_env = root.create_op_env().await.unwrap();
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
    // ????????????op_env?????????????????????
    let op_env1 = root.create_op_env().await.unwrap();
    let op_env2 = root.create_op_env().await.unwrap();

    // op_env1??????remove??????
    let value = op_env1
        .remove_with_key("/a/b/c", "test1", &None)
        .await
        .unwrap();
    assert_eq!(value, Some(x1_value));

    // op_env2??????????????????
    let value = op_env2
        .set_with_key("/a/b/c", "test1", &x1_value2, &None, false)
        .await
        .unwrap();
    assert_eq!(value, Some(x1_value));

    // op_env2?????????
    let new_dec_root = op_env2.commit().await.unwrap();
    info!(
        "op_env2 commit success! dec root {}->{}",
        dec_root, new_dec_root
    );

    // op_env2???????????????????????????????????????????????????????????????
    let ret = op_env1.commit().await;
    assert!(ret.is_err());
    let e = ret.unwrap_err();
    assert_eq!(e.code(), BuckyErrorCode::Conflict);
}

// ???????????????????????????????????????
async fn test_merge(global_state_manager: &GlobalStateManager, dec_id: &ObjectId) {
    info!("will test merge op_env for dec={}", dec_id);

    let root = global_state_manager
        .get_dec_root_manager(&dec_id, true)
        .await
        .unwrap();

    // ??????????????????
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // ????????????
    // /a/b/d/test1 = x1_value
    let op_env = root.create_op_env().await.unwrap();
    op_env
        .insert_with_key("/a/b/d", "test1", &x1_value)
        .await
        .unwrap();

    let dec_root = op_env.commit().await.unwrap();
    // ????????????op_env?????????????????????
    let op_env1 = root.create_op_env().await.unwrap();
    let op_env2 = root.create_op_env().await.unwrap();

    // op_env1??????test1
    let value = op_env1
        .set_with_key("/a/b/d", "test1", &x1_value2, &Some(x1_value), false)
        .await
        .unwrap();
    assert_eq!(value, Some(x1_value));

    // op_env2????????????test2????????????test1
    let value = op_env2
        .set_with_key("/a/b/d", "test2", &x1_value2, &None, true)
        .await
        .unwrap();
    assert!(value.is_none());

    // op_env1??????
    let new_dec_root = op_env1.commit().await.unwrap();
    info!(
        "op_env1 commit success! dec root {}->{}",
        dec_root, new_dec_root
    );

    assert_eq!(new_dec_root, root.get_current_root());

    // op_env2?????????
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

    // ??????????????????
    let x1_value = ObjectId::from_str("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
    let x1_value2 = ObjectId::from_str("95RvaS5aZKKM8ghTYmsTyhSEWD4pAmALoUSJx1yNxSx5").unwrap();

    // ????????????
    // /a/b/d/test1 = x1_value
    let op_env = root.create_op_env().await.unwrap();
    let op_env2 = root.create_op_env().await.unwrap();

    op_env
        .lock_path(vec!["/a/b".to_owned()], 0, true)
        .await
        .unwrap();

    // ??????????????????????????????????????????????????????????????????
    op_env
        .insert_with_key("/a/b/e", "test1", &x1_value)
        .await
        .unwrap();

    // ???????????????????????????????????????????????????
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

    // ????????????????????????
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

    // ?????????????????????
    op_env2
        .lock_path(vec!["/a/c".to_owned(), "/a/b".to_owned()], 0, true)
        .await
        .unwrap_err();

    // op_env???commit??????abort???????????????????????????lock
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

    let env = root.create_op_env().await.unwrap();

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
    info!("new dec root is: {:?}", root1);

    let env2 = root.create_op_env().await.unwrap();
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

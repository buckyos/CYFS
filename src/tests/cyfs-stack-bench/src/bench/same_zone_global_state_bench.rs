use std::sync::Arc;
use async_trait::async_trait;
use crate::{Bench, DEVICE_DEC_ID, Stat};
use log::*;
use cyfs_base::*;
use cyfs_lib::*;
use super::constant::*;

pub struct SameZoneGlobalStateBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
}

pub const GLOABL_STATE_ALL_IN_ONE: &str = "global-state-all-in-one";
pub const ROOT_STATE_MAP_SET:&str = "root-state-inner-zone-map-set-all-in-one";
pub const LOCAL_CACHE_MAP_SET: &str = "local-cache-map-set-all-in-one";

pub const ROOT_STATE_CREATE_NEW_OPERATION: &str = "root-state-inner-zone-create-new-operation";
pub const ROOT_STATE_GET_OPERATION: &str = "root-state-inner-zone-get-operation";
pub const ROOT_STATE_REMOVE_OPERATION: &str = "root-state-inner-zone-remove-operation";
pub const ROOT_STATE_INSERT_OPERATION: &str = "root-state-inner-zone-insert-operation";
pub const ROOT_STATE_COMMIT_OPERATION: &str = "root-state-inner-zone-commit-operation";

pub const LOCAL_CACHE_CREATE_NEW_OPERATION: &str = "local-cache-inner-zone-create-new";
pub const LOCAL_CACHE_GET_OPERATION: &str = "local-cache-inner-zone-get";
pub const LOCAL_CACHE_REMOVE_OPERATION: &str = "local-cache-inner-zone-remove";
pub const LOCAL_CACHE_INSERT_OPERATION: &str = "local-cache-inner-zone-insert";
pub const LOCAL_CACHE_COMMIT_OPERATION: &str = "local-cache-inner-zone-commit";


const LIST: [&str;13] = [
    GLOABL_STATE_ALL_IN_ONE,
    ROOT_STATE_MAP_SET,
    LOCAL_CACHE_MAP_SET,

    ROOT_STATE_CREATE_NEW_OPERATION,
    ROOT_STATE_GET_OPERATION,
    ROOT_STATE_REMOVE_OPERATION,
    ROOT_STATE_INSERT_OPERATION,
    ROOT_STATE_COMMIT_OPERATION,

    LOCAL_CACHE_CREATE_NEW_OPERATION,
    LOCAL_CACHE_GET_OPERATION,
    LOCAL_CACHE_REMOVE_OPERATION,
    LOCAL_CACHE_INSERT_OPERATION,
    LOCAL_CACHE_COMMIT_OPERATION,
];

#[async_trait]
impl Bench for SameZoneGlobalStateBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await?;
        Ok(())
    }

    fn name(&self) -> &str {
        "Same Zone Global State Bench"
    }

    fn print_list(&self) -> Option<&[&str]> {
        Some(&LIST)
    }
}


impl SameZoneGlobalStateBench {
    pub fn new(stack: SharedCyfsStack, target: Option<ObjectId>, stat: Arc<Stat>, run_times: usize) -> Box<Self> {
        Box::new(Self {
            run_times,
            stack,
            target,
            stat
        })
    }

    pub async fn test(&mut self) -> BuckyResult<()> {
        info!("begin test {}", GLOABL_STATE_ALL_IN_ONE);
        for i in 0..self.run_times {
            let begin = std::time::Instant::now();
            self.test_root_state(i).await?;
            self.test_local_cache(i).await?;
            self.stat.write(self.name(), GLOABL_STATE_ALL_IN_ONE, begin.elapsed().as_millis() as u64);
        }

        Ok(())
    }

    // 测试root-state的同zone的跨dec操作 需要配合权限
    async fn test_root_state(&self, _i: usize) -> BuckyResult<()> {
        let begin_root = std::time::Instant::now();
        let root_state = self.stack.root_state_stub(None, Some(DEVICE_DEC_ID.clone()));
        let root_info = root_state.get_current_root().await.unwrap();
        debug!("current root: {:?}", root_info);

        let begin = std::time::Instant::now();
        let access = RootStateOpEnvAccess::new(GLOABL_STATE_PATH, AccessPermissions::ReadAndWrite);   // 对跨dec路径操作这个perm才work
        let op_env = root_state.create_path_op_env_with_access(Some(access)).await.unwrap();
        self.stat.write(self.name(), ROOT_STATE_CREATE_NEW_OPERATION, begin.elapsed().as_millis() as u64);

        let x1_value = ObjectId::from_base58("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
        let x2_value = ObjectId::from_base58("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();

        op_env.remove_with_path("/global-states", None).await.unwrap();

        // test create_new 操作Map
        op_env.remove_with_path("/global-states/new", None).await.unwrap();
        op_env
            .create_new_with_path("/global-states/new/a", ObjectMapSimpleContentType::Map)
            .await
            .unwrap();
        op_env
            .create_new_with_path("/global-states/set", ObjectMapSimpleContentType::Set)
            .await
            .unwrap();

        // if let Err(e) = op_env
        //     .create_new_with_path("/global-states/new/a", ObjectMapSimpleContentType::Map)
        //     .await
        // {
        //     assert!(e.code() == BuckyErrorCode::AlreadyExists);
        // } else {
        //     unreachable!();
        // }

        let begin = std::time::Instant::now();
        // 首先移除老的值，如果存在的话
        op_env.remove_with_path("/global-states/x/b", None).await.unwrap();
        self.stat.write(self.name(), ROOT_STATE_REMOVE_OPERATION, begin.elapsed().as_millis() as u64);

        let ret = op_env.get_by_path("/global-states/x/b").await.unwrap();
        assert_eq!(ret, None);
        let ret = op_env.get_by_path("/global-states/x/b/c").await.unwrap();
        assert_eq!(ret, None);

        let begin = std::time::Instant::now();
        op_env
            .insert_with_key("/global-states/x/b", "c", &x1_value)
            .await
            .unwrap();
        self.stat.write(self.name(), ROOT_STATE_INSERT_OPERATION, begin.elapsed().as_millis() as u64);

        let begin = std::time::Instant::now();
        let ret = op_env.get_by_path("/global-states/x/b/c").await.unwrap();
        assert_eq!(ret, Some(x1_value));
        self.stat.write(self.name(), ROOT_STATE_GET_OPERATION, begin.elapsed().as_millis() as u64);

        let ret = op_env.remove_with_path("/global-states/x/b/d", None).await.unwrap();
        assert_eq!(ret, None);

        let begin = std::time::Instant::now();

        // 操作 Set
        op_env.remove_with_path("/global-states/set", None).await.unwrap();

        let ret = op_env.insert("/global-states/set/a", &x2_value).await.unwrap();
        assert!(ret);

        let ret = op_env.contains("/global-states/set/a", &x1_value).await.unwrap();
        assert!(!ret);

        let ret = op_env.insert("/global-states/set/a", &x1_value).await.unwrap();
        assert!(ret);

        let ret = op_env.insert("/global-states/set/a", &x1_value).await.unwrap();
        assert!(!ret);

        let ret = op_env.remove("/global-states/set/a", &x1_value).await.unwrap();
        assert!(ret);

        let ret = op_env.insert("/global-states/set/a", &x1_value).await.unwrap();
        assert!(ret);

        let root = op_env.commit().await.unwrap();
        self.stat.write(self.name(), ROOT_STATE_COMMIT_OPERATION, begin.elapsed().as_millis() as u64);

        debug!("new dec root is: {:?}", root);

        
        self.stat.write(self.name(), ROOT_STATE_MAP_SET, begin_root.elapsed().as_millis() as u64);

        Ok(())

    }


    async fn test_local_cache(&self, _i: usize) -> BuckyResult<()> {
        let begin_root = std::time::Instant::now();
        let local_cache = self.stack.local_cache_stub(None);
        let root_info = local_cache.get_current_root().await.unwrap();
        debug!("current root: {:?}", root_info);

        let begin = std::time::Instant::now();
        let access = RootStateOpEnvAccess::new(GLOABL_STATE_PATH, AccessPermissions::Full);   // 对跨dec路径操作这个perm才work
        let op_env = local_cache.create_path_op_env_with_access(Some(access)).await.unwrap();
        self.stat.write(self.name(), LOCAL_CACHE_CREATE_NEW_OPERATION, begin.elapsed().as_millis() as u64);

        let x1_value = ObjectId::from_base58("95RvaS5anntyAoRUBi48vQoivWzX95M8xm4rkB93DdSt").unwrap();
        let x2_value = ObjectId::from_base58("95RvaS5F94aENffFhjY1FTXGgby6vUW2AkqWYhtzrtHz").unwrap();

        op_env.remove_with_path("/global-states", None).await.unwrap();

        // test create_new 操作Map
        op_env.remove_with_path("/global-states/new", None).await.unwrap();
        op_env
            .create_new_with_path("/global-states/new/a", ObjectMapSimpleContentType::Map)
            .await
            .unwrap();
        op_env
            .create_new_with_path("/global-states/set", ObjectMapSimpleContentType::Set)
            .await
            .unwrap();

        // if let Err(e) = op_env
        //     .create_new_with_path("/global-states/new/a", ObjectMapSimpleContentType::Map)
        //     .await
        // {
        //     assert!(e.code() == BuckyErrorCode::AlreadyExists);
        // } else {
        //     unreachable!();
        // }

        let begin = std::time::Instant::now();
        // 首先移除老的值，如果存在的话
        op_env.remove_with_path("/global-states/x/b", None).await.unwrap();
        self.stat.write(self.name(), LOCAL_CACHE_REMOVE_OPERATION, begin.elapsed().as_millis() as u64);

        let ret = op_env.get_by_path("/global-states/x/b").await.unwrap();
        assert_eq!(ret, None);
        let ret = op_env.get_by_path("/global-states/x/b/c").await.unwrap();
        assert_eq!(ret, None);

        let begin = std::time::Instant::now();
        op_env
            .insert_with_key("/global-states/x/b", "c", &x1_value)
            .await
            .unwrap();
        self.stat.write(self.name(), LOCAL_CACHE_INSERT_OPERATION, begin.elapsed().as_millis() as u64);

        let begin = std::time::Instant::now();
        let ret = op_env.get_by_path("/global-states/x/b/c").await.unwrap();
        assert_eq!(ret, Some(x1_value));
        self.stat.write(self.name(), LOCAL_CACHE_GET_OPERATION, begin.elapsed().as_millis() as u64);

        let ret = op_env.remove_with_path("/global-states/x/b/d", None).await.unwrap();
        assert_eq!(ret, None);

        // 操作 Set
        op_env.remove_with_path("/global-states/set", None).await.unwrap();

        let ret = op_env.insert("/global-states/set/a", &x2_value).await.unwrap();
        assert!(ret);

        let ret = op_env.contains("/global-states/set/a", &x1_value).await.unwrap();
        assert!(!ret);

        let ret = op_env.insert("/global-states/set/a", &x1_value).await.unwrap();
        assert!(ret);

        let ret = op_env.insert("/global-states/set/a", &x1_value).await.unwrap();
        assert!(!ret);

        let ret = op_env.remove("/global-states/set/a", &x1_value).await.unwrap();
        assert!(ret);

        let ret = op_env.insert("/global-states/set/a", &x1_value).await.unwrap();
        assert!(ret);

        let begin = std::time::Instant::now();
        let root = op_env.commit().await.unwrap();
        self.stat.write(self.name(), LOCAL_CACHE_COMMIT_OPERATION, begin.elapsed().as_millis() as u64);

        debug!("new dec root is: {:?}", root);

        
        self.stat.write(self.name(), LOCAL_CACHE_MAP_SET, begin_root.elapsed().as_millis() as u64);

        Ok(())

    }
}
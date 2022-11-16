use std::sync::Arc;
use async_trait::async_trait;
use crate::{Bench, OOD_DEC_ID, Stat};
use log::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use crate::post_service::ROOT_STATE_CALL_PATH;
use crate::util::new_object;

pub struct CrossZoneRootStateBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
}

const LIST: [&str;2] = ["get_object_by_key", "list"];

#[async_trait]
impl Bench for CrossZoneRootStateBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await?;
        Ok(())
    }

    fn name(&self) -> &str {
        "CrossZone Root State Bench"
    }
    fn print_list(&self) -> Option<&[&str]> {
        Some(&LIST)
    }
}

impl CrossZoneRootStateBench {
    pub fn new(stack: SharedCyfsStack, target: Option<ObjectId>, stat: Arc<Stat>, run_times: usize) -> Box<Self> {
        Box::new(Self {
            run_times,
            stack,
            target,
            stat,
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        let _ = self.add_objects().await?;
        info!("begin test get_object_by_key");
        for _i in 0..self.run_times {
            self.test_get_object_by_key().await?;
        }

        self.remove_objects().await?;
        Ok(())
    }

    async fn add_objects(&self) -> BuckyResult<()> {
        let q = new_object("add", &self.run_times.to_string());

        let mut req = NONPostObjectOutputRequest::new_router(self.target.clone(), q.desc().calculate_id(), q.to_vec().unwrap());

        let req_path = RequestGlobalStatePath::new(Some(OOD_DEC_ID.clone()), Some(ROOT_STATE_CALL_PATH.to_owned()));
        req.common.req_path = Some(req_path.to_string());

        let ret = self.stack.non_service().post_object(req.clone()).await?;
        let t = Text::clone_from_slice(&ret.object.unwrap().object_raw).unwrap();
        assert_eq!(t.header(), "finish");
        Ok(())
    }

    // delete_object only allow within the same zone, use post_object driven target delete operation
    async fn remove_objects(&self) -> BuckyResult<()> {
        let q = new_object("remove", &self.run_times.to_string());

        let mut req = NONPostObjectOutputRequest::new_router(self.target.clone(), q.desc().calculate_id(), q.to_vec().unwrap());

        let req_path = RequestGlobalStatePath::new(Some(OOD_DEC_ID.clone()), Some(ROOT_STATE_CALL_PATH.to_owned()));
        req.common.req_path = Some(req_path.to_string());

        let ret = self.stack.non_service().post_object(req.clone()).await?;
        let t = Text::clone_from_slice(&ret.object.unwrap().object_raw).unwrap();
        assert_eq!(t.header(), "finish");
        Ok(())
    }

    async fn test_get_object_by_key(&self) -> BuckyResult<()> {
        //let root_state = self.stack.root_state_stub(self.target.clone(), Some(OOD_DEC_ID.clone()));
        //let root_info = root_state.get_current_root().await.unwrap();
        //debug!("current root: {:?}", root_info);
        // match root_state.get_current_root().await {
        //     Err(e) => {
        //         assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        //     }
        //     Ok(_) => {
        //         unreachable!();
        //     }
        // }
    
        // match root_state.create_path_op_env().await {
        //     Err(e) => {
        //         assert_eq!(e.code(), BuckyErrorCode::PermissionDenied);
        //     }
        //     Ok(_) => {
        //         unreachable!();
        //     }
        // }

        // let begin = std::time::Instant::now();
        // let access = RootStateOpEnvAccess::new(GLOABL_STATE_PATH, AccessPermissions::ReadAndWrite);   // 对跨dec路径操作这个perm才work
        // let op_env = root_state.create_path_op_env_with_access(Some(access)).await.unwrap();

        // let ret = op_env.get_by_path("/global-states/x/b").await.unwrap();
        // self.stat.write(self.name(),"get_object_by_key", begin.elapsed().as_millis() as u64);

        // info!("ret: {:?}", ret);
        // let list = op_env.list("/global-states/x/b").await.unwrap();
        // self.stat.write(self.name(),"list", begin.elapsed().as_millis() as u64);
        // info!("list: {:?}", list);

        Ok(())
    }

}
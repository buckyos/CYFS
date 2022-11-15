use std::sync::Arc;
use async_trait::async_trait;
use crate::{Bench, DEC_ID2, Stat};
use log::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use crate::post_service::{CALL_PATH, NON_CALL_PATH};
use crate::util::new_object;

pub struct CrossZoneNONBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
    objects: Vec<ObjectId>,
}

const LIST: [&str;2] = ["get-object", "post-object"];

#[async_trait]
impl Bench for CrossZoneNONBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await?;
        Ok(())
    }

    fn name(&self) -> &str {
        "CrossZone NON Bench"
    }
    fn print_list(&self) -> Option<&[&str]> {
        Some(&LIST)
    }
}

impl CrossZoneNONBench {
    pub fn new(stack: SharedCyfsStack, target: Option<ObjectId>, stat: Arc<Stat>, run_times: usize) -> Box<Self> {
        Box::new(Self {
            run_times,
            stack,
            target,
            stat,
            objects: Vec::with_capacity(run_times),
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        info!("let cross ood add objs...");
        let ids = self.add_objects().await?;
        info!("cross ood add {} objs", ids.len());
        info!("begin test get");
        for id in &ids {
            self.test_get_object(id.clone()).await?;
        }

        info!("begin test post");
        self.test_post_object().await?;

        info!("let cross ood remove objs...");
        self.remove_objects(ids).await?;
        Ok(())
    }

    async fn add_objects(&self) -> BuckyResult<Vec<ObjectId>> {
        let q = new_object("add", &self.run_times.to_string());

        let mut req = NONPostObjectOutputRequest::new_router(self.target.clone(), q.desc().calculate_id(), q.to_vec().unwrap());

        let req_path = RequestGlobalStatePath::new(Some(DEC_ID2.clone()), Some(NON_CALL_PATH.to_owned()));
        req.common.req_path = Some(req_path.to_string());

        let ret = self.stack.non_service().post_object(req.clone()).await?;
        let t = Text::clone_from_slice(&ret.object.unwrap().object_raw).unwrap();
        assert_eq!(t.header(), "finish");
        let ids = Vec::<ObjectId>::clone_from_hex(t.value(), &mut vec![]).unwrap();
        Ok(ids)
    }

    // delete_object only allow within the same zone, use post_object driven target delete operation
    async fn remove_objects(&self, ids: Vec<ObjectId>) -> BuckyResult<()> {
        let mut q = new_object("remove", &self.run_times.to_string());
        *q.body_mut_expect("").content_mut().value_mut() = ids.to_hex().unwrap();

        let mut req = NONPostObjectOutputRequest::new_router(self.target.clone(), q.desc().calculate_id(), q.to_vec().unwrap());

        let req_path = RequestGlobalStatePath::new(Some(DEC_ID2.clone()), Some(NON_CALL_PATH.to_owned()));
        req.common.req_path = Some(req_path.to_string());

        let ret = self.stack.non_service().post_object(req.clone()).await?;
        let t = Text::clone_from_slice(&ret.object.unwrap().object_raw).unwrap();
        assert_eq!(t.header(), "finish");
        Ok(())
    }

    async fn test_get_object(&self, id: ObjectId) -> BuckyResult<()> {
        let req =
            NONGetObjectOutputRequest::new_router(self.target.clone(), id, None);
        // req.common.req_path = Some(RequestGlobalStatePath::new(Some(DEC_ID2.clone()), Some(NON_OBJECT_PATH)).format_string());
        let begin = std::time::Instant::now();
        let _ = self.stack.non_service().get_object(req).await?;
        self.stat.write(self.name(),"get-object", begin.elapsed().as_millis() as u64);

        Ok(())
    }

    async fn test_post_object(&self) -> BuckyResult<()> {
        // post_object (device1, dec1) -> (decvice2, dec2)

        for i in 0..self.run_times {
            let begin = std::time::Instant::now();
            let q = new_object("question", &i.to_string());

            let mut req = NONPostObjectOutputRequest::new_router(self.target.clone(), q.desc().calculate_id(), q.to_vec().unwrap());

            let req_path = RequestGlobalStatePath::new(Some(DEC_ID2.clone()), Some(CALL_PATH.to_owned()));
            req.common.req_path = Some(req_path.to_string());

            let ret = self.stack.non_service().post_object(req.clone()).await?;
            let t = Text::clone_from_slice(&ret.object.unwrap().object_raw).unwrap();
            assert_eq!(t.header(), q.header());

            self.stat.write(self.name(),"post-object", begin.elapsed().as_millis() as u64);
        }

        Ok(())
    }
}
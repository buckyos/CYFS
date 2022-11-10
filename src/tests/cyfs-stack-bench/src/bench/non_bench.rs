use std::sync::Arc;
use async_trait::async_trait;
use crate::{Bench, Stat};
use log::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use crate::post_service::CALL_PATH;
use crate::util::new_object;
use super::constant::*;

pub struct NONBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
    objects: Vec<(ObjectId, Text)>
}

#[async_trait]
impl Bench for NONBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        info!("begin test NONBench...");
        let begin = std::time::Instant::now();
        self.test().await?;
        let dur = begin.elapsed();
        info!("end test NONBench: {:?}", dur);
        let costs = begin.elapsed().as_millis() as u64;
        self.stat.write(NON_ALL_IN_ONE, costs);
        Ok(())
    }

    fn name(&self) -> &str {
        "NON Bench"
    }
}

impl NONBench {
    pub fn new(stack: SharedCyfsStack, target: Option<ObjectId>, stat: Arc<Stat>, run_times: usize) -> Box<Self> {
        Box::new(Self {
            run_times,
            stack,
            target,
            stat,
            objects: Vec::with_capacity(run_times)
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        self.gen_objects();
        self.test_put_object().await?;
        self.test_get_object().await?;

        self.test_delete_object().await?;
        self.test_post_object().await?;

        Ok(())
    }

    fn gen_objects(&mut self) {
        info!("generating test objects...");
        for i in 0..self.run_times {
            let obj = new_object("obj", &i.to_string());
            self.objects.push((obj.desc().calculate_id(), obj))
        }
    }

    async fn test_put_object(&self) -> BuckyResult<()> {
        info!("begin test_put_object...");
        let begin = std::time::Instant::now();

        for i in 0..self.run_times {
            let req =
                NONPutObjectOutputRequest::new_router(self.target.clone(), self.objects[i].0.clone(), self.objects[i].1.to_vec().unwrap());
            let begin = std::time::Instant::now();
            let _ = self.stack.non_service().put_object(req).await?;
            let dur = begin.elapsed();
            self.stat.write(NON_PUT_OBJECT, dur.as_millis() as u64);
        }

        let dur = begin.elapsed();
        info!("end test_put_object: {:?}", dur);
        Ok(())
    }

    async fn test_get_object(&self) -> BuckyResult<()> {
        info!("begin test_get_object...");
        let begin = std::time::Instant::now();

        for i in 0..self.run_times {
            let req =
                NONGetObjectOutputRequest::new_router(self.target.clone(), self.objects[i].0.clone(), None);
            let begin = std::time::Instant::now();
            let _ = self.stack.non_service().get_object(req).await?;
            self.stat.write(NON_GET_OBJECT, begin.elapsed().as_millis() as u64);
        }

        let dur = begin.elapsed();
        info!("end test_get_object: {:?}", dur);

        Ok(())
    }

    async fn test_delete_object(&self) -> BuckyResult<()> {
        info!("begin test_delete_object...");
        let begin = std::time::Instant::now();

        for i in 0..self.run_times {
            let req =
                NONDeleteObjectOutputRequest::new_router(self.target.clone(), self.objects[i].0.clone(), None);
            let begin = std::time::Instant::now();
            let _ = self.stack.non_service().delete_object(req).await?;
            self.stat.write(NON_DELETE_OBJECT, begin.elapsed().as_millis() as u64);
        }

        let dur = begin.elapsed();
        info!("end test_delete_object: {:?}", dur);

        Ok(())
    }

    async fn test_post_object(&self) -> BuckyResult<()> {
        info!("begin test_post_object...");
        let begin = std::time::Instant::now();
        // post_object (device1, dec1) -> (decvice2, dec2)

        for i in 0..self.run_times {
            let begin = std::time::Instant::now();
            let q = new_object("question", &i.to_string());

            let mut req = NONPostObjectOutputRequest::new_router(self.target.clone(), q.desc().calculate_id(), q.to_vec().unwrap());

            let req_path = RequestGlobalStatePath::new(None, Some(CALL_PATH.to_owned()));
            req.common.req_path = Some(req_path.to_string());

            let ret = self.stack.non_service().post_object(req.clone()).await?;
            let t = Text::clone_from_slice(&ret.object.unwrap().object_raw).unwrap();
            assert_eq!(t.value(), &i.to_string());

            self.stat.write(NON_POST_OBJECT, begin.elapsed().as_millis() as u64);
        }

        let dur = begin.elapsed();
        info!("end test_post_object: {:?}", dur);

        Ok(())
    }
}
use std::sync::Arc;
use async_trait::async_trait;
use crate::{Bench, DEC_ID2, Stat};
use log::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use crate::bench::NON_OBJECT_PATH;
use crate::post_service::CALL_PATH;
use crate::util::new_object;

pub struct SameZoneNONBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
    objects: Vec<(ObjectId, Text)>,
}

const LIST: [&str;6] = ["put-object", "get-object", "delete-object-from-ood", "delete-object-from-local", "put-get-delete", "post-object"];

#[async_trait]
impl Bench for SameZoneNONBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await?;
        Ok(())
    }

    fn name(&self) -> &str {
        "SameZone NON Bench"
    }
    fn print_list(&self) -> Option<&[&str]> {
        Some(&LIST)
    }
}

impl SameZoneNONBench {
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
        self.gen_objects();
        info!("begin test put-get-delete");
        for i in 0..self.run_times {
            let begin = std::time::Instant::now();
            self.test_put_object(i).await?;
            self.test_get_object(i).await?;
            self.test_delete_object(i).await?;
            self.stat.write(self.name(), "put-get-delete", begin.elapsed().as_millis() as u64);
        }

        info!("begin test post");
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

    async fn test_put_object(&self, i: usize) -> BuckyResult<()> {
        let mut req =
            NONPutObjectOutputRequest::new_router(self.target.clone(), self.objects[i].0.clone(), self.objects[i].1.to_vec().unwrap());
        req.common.req_path = Some(RequestGlobalStatePath::new(Some(DEC_ID2.clone()), Some(NON_OBJECT_PATH)).format_string());
        req.access = Some(AccessString::full());
        let begin = std::time::Instant::now();
        let _ = self.stack.non_service().put_object(req).await?;
        self.stat.write(self.name(), "put-object", begin.elapsed().as_millis() as u64);

        Ok(())
    }

    async fn test_get_object(&self, i: usize) -> BuckyResult<()> {
        let req =
            NONGetObjectOutputRequest::new_router(self.target.clone(), self.objects[i].0.clone(), None);
        // req.common.req_path = Some(RequestGlobalStatePath::new(Some(DEC_ID2.clone()), Some(NON_OBJECT_PATH)).format_string());
        let begin = std::time::Instant::now();
        let _ = self.stack.non_service().get_object(req).await?;
        self.stat.write(self.name(),"get-object", begin.elapsed().as_millis() as u64);

        Ok(())
    }

    async fn test_delete_object(&self, i: usize) -> BuckyResult<()> {
        let req =
            NONDeleteObjectOutputRequest::new_router(self.target.clone(), self.objects[i].0.clone(), None);
        // req.common.req_path = Some(RequestGlobalStatePath::new(Some(DEC_ID2.clone()), Some(NON_OBJECT_PATH)).format_string());
        let begin = std::time::Instant::now();
        let _ = self.stack.non_service().delete_object(req).await?;
        self.stat.write(self.name(),"delete-object-from-ood", begin.elapsed().as_millis() as u64);

        let begin = std::time::Instant::now();
        let _ = self.stack.non_service().delete_object(NONDeleteObjectOutputRequest::new_noc(self.objects[i].0.clone(), None)).await?;
        self.stat.write(self.name(),"delete-object-from-local", begin.elapsed().as_millis() as u64);

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
use std::sync::Arc;
use async_trait::async_trait;
use cyfs_core::{Text, TextObj};
use crate::{Bench, OOD_DEC_ID, Stat};
use log::*;
use cyfs_base::*;
use cyfs_lib::*;
use crate::post_service::NDN_CALL_PATH;
use crate::util::new_object;

pub struct CrossZoneNDNBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
    chunks: Vec<ChunkId>,
}

const LIST: [&str;1] = ["get-chunk"];

#[async_trait]
impl Bench for CrossZoneNDNBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await
    }

    fn name(&self) -> &str {
        "CrossZone NDN Bench"
    }

    fn print_list(&self) -> Option<&[&str]> {
        Some(&LIST)
    }
}

impl CrossZoneNDNBench {
    pub fn new(stack: SharedCyfsStack, target: Option<ObjectId>, stat: Arc<Stat>, run_times: usize) -> Box<Self> {
        Box::new(Self {
            run_times,
            stack,
            target,
            stat,
            chunks: Vec::with_capacity(run_times),
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        info!("let cross ood add chunks...");
        let ids = self.add_chunks().await?;
        info!("cross ood add {} chunks", ids.len());
        for id in &ids {
            //self.test_get_chunk(id.clone()).await?;
        }

        //info!("let cross ood remove objs...");
        self.remove_chunks(ids).await?;
        Ok(())
    }


    async fn add_chunks(&self) -> BuckyResult<Vec<ChunkId>> {
        let q = new_object("add_chunk", &self.run_times.to_string());

        let mut req = NONPostObjectOutputRequest::new_router(self.target.clone(), q.desc().calculate_id(), q.to_vec().unwrap());

        let req_path = RequestGlobalStatePath::new(Some(OOD_DEC_ID.clone()), Some(NDN_CALL_PATH.to_owned()));
        req.common.req_path = Some(req_path.to_string());

        let ret = self.stack.non_service().post_object(req.clone()).await?;
        let t = Text::clone_from_slice(&ret.object.unwrap().object_raw).unwrap();
        assert_eq!(t.header(), "finish");
        let ids = Vec::<ChunkId>::clone_from_hex(t.value(), &mut vec![]).unwrap();
        Ok(ids)
    }

    async fn remove_chunks(&self, _ids: Vec<ChunkId>) -> BuckyResult<()> {
        Ok(())
    }

    async fn test_get_chunk(&self, chunk_id: ChunkId) -> BuckyResult<()> {
        let device_id = DeviceId::try_from(self.target.clone().unwrap()).unwrap();
        info!("begin test-get-chunk...");
        let req = NDNGetDataRequest::new_router(
            Some(device_id.clone().into()),
            chunk_id.object_id().to_owned(),
            None,
        );

        //req.common.req_path = Some(RequestGlobalStatePath::new(Some(OOD_DEC_ID.clone()), Some(NDN_CHUNKS_PATH)).format_string());
    
        info!(
            "will get chunk from device: chunk={}, device={}",
            chunk_id, device_id,
        );
        let begin = std::time::Instant::now();

        let resp = self.stack.ndn_service().get_data(req).await.unwrap();
        assert_eq!(resp.object_id, chunk_id.object_id());
        self.stat.write(self.name(),"get-chunk", begin.elapsed().as_millis() as u64);

        Ok(())
    }
}
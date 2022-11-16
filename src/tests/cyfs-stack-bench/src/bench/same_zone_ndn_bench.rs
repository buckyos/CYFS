use async_std::io::prelude::*;
use std::sync::Arc;
use async_trait::async_trait;
use crate::{Bench, Stat, OOD_DEC_ID, bench::NDN_CHUNKS_PATH};
use log::*;
use cyfs_base::*;
use cyfs_lib::*;


pub const NDN_INNER_ZONE_ALL_IN_ONE: &str = "inner-zone-all-in-one";
pub const NDN_INNER_ZONE_PUT_CHUNK: &str = "inner-zone-put-chunk";
pub const NDN_INNER_ZONE_GET_CHUNK: &str = "inner-zone-get-chunk";
//pub const NDN_INNER_ZONE_DELETE_CHUNK: &str = "inner-zone-delete-chunk";

const LIST: [&str;3] = [
    NDN_INNER_ZONE_ALL_IN_ONE,
    NDN_INNER_ZONE_PUT_CHUNK,
    NDN_INNER_ZONE_GET_CHUNK,
    //NDN_INNER_ZONE_DELETE_CHUNK,
];

pub struct SameZoneNDNBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
}

#[async_trait]
impl Bench for SameZoneNDNBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await
    }

    fn name(&self) -> &str {
        "SameZone NDN Bench"
    }

    fn print_list(&self) -> Option<&[&str]> {
        Some(&LIST)
    }
}

impl SameZoneNDNBench {
    pub fn new(stack: SharedCyfsStack, target: Option<ObjectId>, stat: Arc<Stat>, run_times: usize) -> Box<Self> {
        Box::new(Self {
            run_times,
            stack,
            target,
            stat,
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        let begin = std::time::Instant::now();
        info!("begin test-put-get chunk...");
        for _i in 0..self.run_times {
            let ret = self.test_put_chunk().await;
            //let ret2 = ret.clone();
            self.test_get_chunk(ret.0, ret.1, ret.2).await;
            //self.test_delete_chunk(ret2.0, ret2.2).await;
        }

        self.stat.write(self.name(),NDN_INNER_ZONE_ALL_IN_ONE, begin.elapsed().as_millis() as u64);

        Ok(())
    }

    async fn test_put_chunk(&self) -> (ChunkId, Vec<u8>, DeviceId) {
        let begin = std::time::Instant::now();
        info!("begin test-put-chunk...");
        let buf: Vec<u8> = (0..3000).map(|_| rand::random::<u8>()).collect();
        let chunk_id = ChunkId::calculate(&buf).await.unwrap();

        let mut req = NDNPutDataRequest::new_with_buffer(
            NDNAPILevel::Router,
            chunk_id.object_id().to_owned(),
            buf.clone(),
        );
        req.common.target = Some(self.stack.local_device_id().into());
        req.common.req_path = Some(RequestGlobalStatePath::new(Some(OOD_DEC_ID.clone()), Some(NDN_CHUNKS_PATH)).format_string());
        if let Err(e) = self.stack.ndn_service().put_data(req).await {
            error!("put chunk error! {}", e);
            unreachable!();
        }
        self.stat.write(self.name(),NDN_INNER_ZONE_PUT_CHUNK, begin.elapsed().as_millis() as u64);

        // 立即get一次
        {
            let req = NDNGetDataRequest::new_ndc(chunk_id.object_id().to_owned(), None);

            let mut resp = self.stack.ndn_service().get_data(req).await.unwrap();
            assert_eq!(resp.length as usize, buf.len());

            let mut chunk = vec![];
            let count = resp.data.read_to_end(&mut chunk).await.unwrap();
            assert_eq!(count, resp.length as usize);
            assert_eq!(buf, chunk);
        }

        // 测试exits
        {
            //let req = NDNExistChunkRequest {
            //    chunk_id: chunk_id.to_owned(),
            //};

            //let resp = stack.ndn_service().exist_chunk(req).await.unwrap();
            //assert!(resp.exist);
        }
        let device_id = self.stack.local_device_id();
        //let device_id = DeviceId::try_from(self.target.clone().unwrap()).unwrap();
        (chunk_id, buf, device_id)
    }

    async fn test_get_chunk(&self, chunk_id: ChunkId, chunk: Vec<u8>, device_id: DeviceId) {
        let begin = std::time::Instant::now();
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
    
        let mut resp = self.stack.ndn_service().get_data(req).await.unwrap();
        assert_eq!(resp.object_id, chunk_id.object_id());
    
        self.stat.write(self.name(),NDN_INNER_ZONE_GET_CHUNK, begin.elapsed().as_millis() as u64);

        let mut buf = vec![];
        let size = resp.data.read_to_end(&mut buf).await.unwrap();
        assert_eq!(size, chunk_id.len());
        assert_eq!(size, chunk.len());
        assert_eq!(buf, chunk);
    
        info!(
            "get chunk from device success! file={}, len={}",
            chunk_id, size
        );
    }

    //// UnSupport, ndc delete_data not support yet
    // async fn test_delete_chunk(&self, chunk_id: ChunkId, device_id: DeviceId) {
    //     let begin = std::time::Instant::now();
    //     info!("begin test-delete-chunk...");
    //     let req = NDNDeleteDataRequest::new_router(
    //         Some(device_id.clone().into()),
    //         chunk_id.object_id().to_owned(),
    //         None,
    //     );
    
    //     info!(
    //         "will get chunk from device: chunk={}, device={}",
    //         chunk_id, device_id,
    //     );
    
    //     let resp = self.stack.ndn_service().delete_data(req).await.unwrap();
    //     assert_eq!(resp.object_id, chunk_id.object_id());
    //     self.stat.write(self.name(),NDN_INNER_ZONE_DELETE_CHUNK, begin.elapsed().as_millis() as u64);

    //     info!(
    //         "delete chunk from device success! file={}",
    //         chunk_id
    //     );
    // }

}
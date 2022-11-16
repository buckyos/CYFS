use std::sync::Arc;
use async_trait::async_trait;
use crate::{Bench, OOD_DEC_ID, Stat};
use log::*;
use cyfs_base::*;
use cyfs_core::*;
use cyfs_lib::*;
use crate::post_service::{CALL_PATH, NON_CALL_PATH};
use crate::util::new_object;

pub const NDN_INNER_ZONE_PUT_CHUNK: &str = "inner-zone-put-chunk";
pub const NDN_INNER_ZONE_GET_CHUNK: &str = "inner-zone-get-chunk";
pub const NDN_INNER_ZONE_DELETE_CHUNK: &str = "inner-zone-delete-chunk";

const LIST: [&str;3] = [
    NDN_INNER_ZONE_PUT_CHUNK,
    NDN_INNER_ZONE_GET_CHUNK,
    NDN_INNER_ZONE_DELETE_CHUNK,
];

pub struct SameZoneNDNBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
    objects: Vec<ObjectId>,
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
            objects: Vec::with_capacity(run_times),
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        Ok(())
    }
}
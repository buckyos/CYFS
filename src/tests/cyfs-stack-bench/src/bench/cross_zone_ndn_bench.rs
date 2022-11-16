use std::sync::Arc;
use async_trait::async_trait;
use crate::{Bench, OOD_DEC_ID, Stat};
use log::*;
use cyfs_base::*;
use cyfs_lib::*;
use crate::post_service::{CALL_PATH, NON_CALL_PATH};
use crate::util::new_object;

pub struct CrossZoneNDNBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
    objects: Vec<ObjectId>,
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
            objects: Vec::with_capacity(run_times),
        })
    }
    async fn test(&mut self) -> BuckyResult<()> {
        Ok(())
    }
}
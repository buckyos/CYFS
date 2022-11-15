use async_trait::async_trait;
use crate::{Bench, BenchEnv, sim_zone::SimZone};
use log::*;

pub struct NDNBench {
    run_times: usize,
    stack: SharedCyfsStack,
    target: Option<ObjectId>,
    stat: Arc<Stat>,
    objects: Vec<ObjectId>,
}

#[async_trait]
impl Bench for NDNBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await
    }

    fn name(&self) -> &str {
        "NDN Bench"
    }

    fn print_list(&self) -> Option<&[&str]> {
        None
    }
}

impl NDNBench {
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
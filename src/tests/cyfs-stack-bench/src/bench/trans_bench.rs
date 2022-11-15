use async_trait::async_trait;
use crate::{Bench, BenchEnv, sim_zone::SimZone};
use log::*;

pub struct TransBench {}

#[async_trait]
impl Bench for TransBench {
    async fn bench(&mut self) -> BuckyResult<()> {
        self.test().await
    }

    fn name(&self) -> &str {
        "Trans Bench"
    }

    fn print_list(&self) -> Option<&[&str]> {
        None
    }
}
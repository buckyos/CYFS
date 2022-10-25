use async_trait::async_trait;
use crate::{Bench, BenchEnv};
use log::*;

pub struct TransBench {}

#[async_trait]
impl Bench for TransBench {
    async fn bench(&self, _env: BenchEnv, _t: u64) -> bool {
        true
    }

    fn name(&self) -> &str {
        "Trans Bench"
    }
}
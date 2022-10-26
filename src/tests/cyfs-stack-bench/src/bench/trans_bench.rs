use async_trait::async_trait;
use crate::{Bench, BenchEnv};
use log::*;

pub struct TransBench {}

#[async_trait]
impl Bench for TransBench {
    async fn bench(&self, env: BenchEnv, _ood_path: String, _t: u64) -> bool {
        let ret = if env == BenchEnv::Simulator {
            true
        } else {
            true
        };

        ret
    }

    fn name(&self) -> &str {
        "Trans Bench"
    }
}
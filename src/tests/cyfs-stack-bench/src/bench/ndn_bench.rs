use async_trait::async_trait;
use crate::{Bench, BenchEnv};
use log::*;

pub struct NDNBench {}

#[async_trait]
impl Bench for NDNBench {
    async fn bench(&self, env: BenchEnv, _ood_path: String, _t: u64) -> bool {
        let ret = if env == BenchEnv::Simulator {
            true
        } else {
            true
        };

        ret
        
    }

    fn name(&self) -> &str {
        "NDN Bench"
    }
}
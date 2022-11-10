use async_trait::async_trait;
use crate::{Bench, BenchEnv, sim_zone::SimZone};
use log::*;

pub struct TransBench {}

#[async_trait]
impl Bench for TransBench {
    async fn bench(&self, env: BenchEnv, _zone: &SimZone, _ood_path: String, _t: u64) -> bool {
        info!("begin test TransBench...");
        let begin = std::time::Instant::now();

        let ret = if env == BenchEnv::Simulator {
            true
        } else {
            // TODO: support physical stack  ood/runtime
            true
        };

        let dur = begin.elapsed();
        info!("end test TransBench: {:?}", dur);

        ret
    }

    fn name(&self) -> &str {
        "Trans Bench"
    }
}
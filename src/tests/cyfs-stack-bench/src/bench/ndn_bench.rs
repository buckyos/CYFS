use async_trait::async_trait;
use crate::Bench;
use log::*;

pub struct NDNBench {}

#[async_trait]
impl Bench for NDNBench {
    async fn bench(&self, _: u64) -> bool {
        true
    }

    fn name(&self) -> &str {
        "NDN Bench"
    }
}
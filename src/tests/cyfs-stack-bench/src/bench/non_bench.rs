use async_trait::async_trait;
use crate::Bench;
use log::*;

pub struct NONBench {}

#[async_trait]
impl Bench for NONBench {
    async fn bench(&self, _: u64) -> bool {
        true
    }

    fn name(&self) -> &str {
        "NON Bench"
    }
}
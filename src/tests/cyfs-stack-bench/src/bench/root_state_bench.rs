use async_trait::async_trait;
use crate::Bench;
use log::*;

pub struct RootStateBench {}

#[async_trait]
impl Bench for RootStateBench {
    async fn bench(&self, _: u64) -> bool {
        true
    }

    fn name(&self) -> &str {
        "Root State Bench"
    }
}
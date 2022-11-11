//mod ndn_bench;
mod same_zone_non_bench;
mod cross_zone_non_bench;
//mod root_state_bench;
//mod trans_bench;
//mod other_bench;
mod constant;

use cyfs_base::BuckyResult;
use async_trait::async_trait;

#[async_trait]
pub(crate) trait Bench {
    async fn bench(&mut self) -> BuckyResult<()>;
    fn name(&self) -> &str;
    fn print_list(&self) -> Option<&[&str]> {None}
}

//pub use ndn_bench::*;
pub use same_zone_non_bench::*;
pub use cross_zone_non_bench::*;
//pub use root_state_bench::*;
//pub use trans_bench::*;
//pub use other_bench::*;
pub use constant::*;
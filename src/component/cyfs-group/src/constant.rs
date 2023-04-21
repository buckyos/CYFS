use std::time::Duration;

pub const GROUP_METHOD_UPDATE: &str = ".update";
pub const GROUP_METHOD_DECIDE: &str = ".decide";
pub enum GroupUpdateDecide {
    Accept = 1,
    Reject = 2,
    JoinAdmin = 3,
    JoinMember = 4,
}

// Some config
pub const NETWORK_TIMEOUT: Duration = Duration::from_millis(5000);
pub const HOTSTUFF_TIMEOUT_DEFAULT: u64 = 5000;
pub const CHANNEL_CAPACITY: usize = 1000;
pub const TIME_PRECISION: Duration = Duration::from_millis(60000);
pub const PROPOSAL_MAX_TIMEOUT: Duration = Duration::from_secs(3600);
pub const SYNCHRONIZER_TIMEOUT: u64 = 500;
pub const SYNCHRONIZER_TRY_TIMES: usize = 3;
pub const CLIENT_POLL_TIMEOUT: Duration = Duration::from_millis(5000);
pub const STATE_NOTIFY_COUNT_PER_ROUND: usize = 8;
pub const NET_PROTOCOL_VPORT: u16 = 2048;
pub const MEMORY_CACHE_SIZE: usize = 1024;
pub const MEMORY_CACHE_DURATION: Duration = Duration::from_secs(300);
pub const GROUP_DEFAULT_CONSENSUS_INTERVAL: u64 = 5000; // default 5000 ms
pub const BLOCK_COUNT_REST_TO_SYNC: u64 = 8; // the node will stop most work, and synchronize the lost blocks.

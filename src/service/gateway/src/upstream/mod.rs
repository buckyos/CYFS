pub mod tcp_up_stream;
pub mod udp_up_stream;
pub mod udp_sender;
mod peer_assoc;

pub use peer_assoc::*;

pub use tcp_up_stream::TcpUpStream;
pub use tcp_up_stream::TcpUpStreamForBdt;

pub use udp_up_stream::{UdpUpStream, UdpUpStreamManager};
pub use udp_sender::*;

use std::time::Duration;

// udp关联的默认超时时间
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60 * 5);

// UDP包的载荷最大大小
pub const MAXIMUM_UDP_PAYLOAD_SIZE: usize = 65536;
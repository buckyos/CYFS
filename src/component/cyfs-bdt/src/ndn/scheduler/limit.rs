
#[derive(Clone)]
pub struct Config{
    pub max_connections_per_source: u16,

    pub max_connections: u16,
    pub max_cpu_usage: u8,
    pub max_memory_usage: u8,
    pub max_upstream_bandwidth: u32,
    pub max_downstream_bandwidth: u32,
}

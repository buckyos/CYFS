mod gateway_query;
mod gateway_register;

pub use gateway_register::GatewayRegister;
pub use gateway_query::GatewayQuery;

pub const GATEWAY_CONTROL_URL: &str = "http://127.0.0.1:1314";
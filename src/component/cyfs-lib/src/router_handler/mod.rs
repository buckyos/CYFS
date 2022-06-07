mod action;
mod category;
mod chain;
mod filter;
mod handler;
mod http;
mod processor;
mod request;
mod ws;
mod dec_checker;

pub use action::*;
pub use category::*;
pub use chain::*;
pub use filter::*;
pub use handler::*;
pub use http::*;
pub use processor::*;
pub use request::*;
pub use ws::*;
pub use dec_checker::*;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct RouterHandlerId {
    chain: RouterHandlerChain,
    category: RouterHandlerCategory,
    id: String,
}

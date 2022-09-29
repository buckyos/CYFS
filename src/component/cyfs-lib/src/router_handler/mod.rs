mod action;
mod category;
mod chain;
mod filter;
mod handler;
mod http;
mod processor;
mod request;
mod ws;

pub use action::*;
pub use category::*;
pub use chain::*;
pub use filter::*;
pub use handler::*;
pub use http::*;
pub use processor::*;
pub use request::*;
pub use ws::*;

#[derive(Clone, Debug, Hash, Eq, PartialEq)]
struct RouterHandlerId {
    chain: RouterHandlerChain,
    category: RouterHandlerCategory,
    id: String,
}

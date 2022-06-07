mod stream_provider;
pub mod package;
pub mod tcp;
pub mod container;
pub mod listener;
mod manager;

#[derive(Clone)]
pub struct Config {
    pub stream: container::Config, 
    pub listener: listener::Config
}

pub use container::{StreamProviderSelector, StreamContainer, StreamGuard, StreamState};
pub use listener::{StreamListener, StreamListenerGuard, StreamListenerState, StreamIncoming};
pub use manager::{StreamManager, WeakStreamManager, RemoteSequence};
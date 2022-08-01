use async_trait::{async_trait};
pub use std::net::Shutdown;
use std::task::{Context, Poll};
use cyfs_base::*;
use crate::protocol::{*, v0::*};
use super::container::StreamContainer;

#[async_trait]
pub trait StreamProvider: std::fmt::Display + Send + Sync {
    fn local_ep(&self) -> &Endpoint;
    fn remote_ep(&self) -> &Endpoint;
    fn start(&self, owner: &StreamContainer);
    fn clone_as_package_handler(&self) -> Option<Box<dyn OnPackage<SessionData>>>;
    fn clone_as_provider(&self) ->Box<dyn StreamProvider>;
    fn shutdown(&self, which: Shutdown, owner: &StreamContainer) -> Result<(), std::io::Error>;

    fn poll_readable(&self, cx: &mut Context<'_>) -> Poll<std::io::Result<usize>>;
    fn poll_read(
        &self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>>;

    fn poll_write(
        &self,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>>;
    fn poll_flush(&self, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>>;
    fn poll_close(&self, _: &mut Context<'_>) -> Poll<std::io::Result<()>>;
}
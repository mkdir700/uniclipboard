use std::time::Duration;
use crate::message::Payload;
use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait RemoteClipboardSync: Send + Sync {
    async fn push(&self, payload: Payload) -> Result<()>;
    async fn pull(&self, timeout: Option<Duration>) -> Result<Payload>;
    async fn sync(&self) -> Result<()>;
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn pause(&self) -> Result<()>;
    async fn resume(&self) -> Result<()>;
}

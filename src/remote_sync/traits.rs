use crate::message::Payload;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
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

#[async_trait]
pub trait RemoteSyncManagerTrait: Send + Sync {
    #[allow(unused)]
    async fn sync(&self) -> Result<()>;
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn pause(&self) -> Result<()>;
    async fn resume(&self) -> Result<()>;
    async fn push(&self, payload: Payload) -> Result<()>;
    async fn pull(&self, timeout: Option<std::time::Duration>) -> Result<Payload>;
    async fn set_sync_handler(&self, handler: Arc<dyn RemoteClipboardSync>);
}

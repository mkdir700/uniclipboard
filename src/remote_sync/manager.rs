use std::{sync::Arc, time::Duration};

use super::traits::RemoteClipboardSync;
use crate::message::Payload;
use anyhow::Result;
use tokio::sync::RwLock;
pub struct RemoteSyncManager {
    sync_handler: Arc<RwLock<Option<Arc<dyn RemoteClipboardSync>>>>,
}
impl RemoteSyncManager {
    pub fn new() -> Self {
        RemoteSyncManager {
            sync_handler: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn set_sync_handler(&self, handler: Arc<dyn RemoteClipboardSync>) {
        let mut sync_handler = self.sync_handler.write().await;
        *sync_handler = Some(handler);
    }

    pub async fn push(&self, payload: Payload) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.push(payload).await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    pub async fn pull(&self, timeout: Option<Duration>) -> Result<Payload> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.pull(timeout).await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    #[allow(dead_code)]
    pub async fn sync(&self) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.sync().await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    pub async fn start(&self) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.start().await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    #[allow(dead_code)]
    pub async fn stop(&self) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.stop().await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }
}
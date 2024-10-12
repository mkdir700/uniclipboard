use std::{sync::Arc, time::Duration};

use super::traits::{RemoteClipboardSync, RemoteSyncManagerTrait};
use crate::message::ClipboardSyncMessage;
use anyhow::Result;
use tokio::sync::RwLock;
use async_trait::async_trait;

pub struct RemoteSyncManager {
    sync_handler: Arc<RwLock<Option<Arc<dyn RemoteClipboardSync>>>>,
}

impl RemoteSyncManager {
    pub fn new() -> Self {
        RemoteSyncManager {
            sync_handler: Arc::new(RwLock::new(None)),
        }
    }
}

#[async_trait]
impl RemoteSyncManagerTrait for RemoteSyncManager {
    async fn set_sync_handler(&self, handler: Arc<dyn RemoteClipboardSync>) {
        let mut sync_handler = self.sync_handler.write().await;
        *sync_handler = Some(handler);
    }

    async fn push(&self, message: ClipboardSyncMessage) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.push(message).await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    async fn pull(&self, timeout: Option<Duration>) -> Result<ClipboardSyncMessage> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.pull(timeout).await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    #[allow(dead_code)]
    async fn sync(&self) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.sync().await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    async fn start(&self) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.start().await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    #[allow(dead_code)]
    async fn stop(&self) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.stop().await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    async fn pause(&self) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.pause().await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }

    async fn resume(&self) -> Result<()> {
        let sync_handler = self.sync_handler.read().await;
        if let Some(handler) = sync_handler.as_ref() {
            handler.resume().await
        } else {
            Err(anyhow::anyhow!("No sync handler set"))
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::remote_sync::traits::MockRemoteClipboardSync;

    use super::*;
    use chrono::Utc;
    use mockall::predicate::*;
    use std::sync::Arc;
    use bytes::Bytes;

    #[tokio::test]
    async fn test_set_sync_handler() {
        let manager = RemoteSyncManager::new();
        let mock_handler = Arc::new(MockRemoteClipboardSync::new());
        manager.set_sync_handler(mock_handler.clone()).await;
        assert!(manager.sync_handler.read().await.is_some());
    }

    // #[tokio::test]
    // async fn test_push() {
    //     let manager = RemoteSyncManager::new();
    //     let mut mock_handler = MockRemoteClipboardSync::new();
    //     let payload = Payload::new_text(
    //         Bytes::from("test_data".to_string()),
    //         "text/plain".to_string(),
    //         Utc::now(),
    //     );
    //     let message = ClipboardSyncMessage::from_payload(payload);
    //     mock_handler
    //         .expect_push()
    //         .with(eq(message.clone()))
    //         .times(1)
    //         .returning(|_| Ok(()));

    //     manager.set_sync_handler(Arc::new(mock_handler)).await;
    //     assert!(manager.push(message).await.is_ok());
    // }

    // #[tokio::test]
    // async fn test_pull() {
    //     let manager = RemoteSyncManager::new();
    //     let mut mock_handler = MockRemoteClipboardSync::new();
    //     let payload = Payload::new_text(
    //         Bytes::from("test_data".to_string()),
    //         "text/plain".to_string(),
    //         Utc::now(),
    //     );

    //     let payload_clone = payload.clone();
    //     mock_handler
    //         .expect_pull()
    //         .with(eq(None))
    //         .times(1)
    //         .returning(move |_| Ok(Some(payload_clone.clone())));

    //     manager.set_sync_handler(Arc::new(mock_handler)).await;
    //     let received_payload = manager.pull(None).await.unwrap().unwrap();
    //     assert_eq!(payload, received_payload);
    // }

    #[tokio::test]
    async fn test_start() {
        let manager = RemoteSyncManager::new();
        let mut mock_handler = MockRemoteClipboardSync::new();
        
        mock_handler
            .expect_start()
            .times(1)
            .returning(|| Ok(()));

        manager.set_sync_handler(Arc::new(mock_handler)).await;
        assert!(manager.start().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_stop() {
        let manager = RemoteSyncManager::new();
        let mut mock_handler = MockRemoteClipboardSync::new();
        
        mock_handler
            .expect_stop()
            .times(1)
            .returning(|| Ok(()));

        manager.set_sync_handler(Arc::new(mock_handler)).await;
        assert!(manager.stop().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_pause() {
        let manager = RemoteSyncManager::new();
        let mut mock_handler = MockRemoteClipboardSync::new();
        
        mock_handler
            .expect_pause()
            .times(1)
            .returning(|| Ok(()));

        manager.set_sync_handler(Arc::new(mock_handler)).await;
        assert!(manager.pause().await.is_ok());
    }
    
    #[tokio::test]
    async fn test_resume() {
        let manager = RemoteSyncManager::new();
        let mut mock_handler = MockRemoteClipboardSync::new();
        
        mock_handler
            .expect_resume()
            .times(1)
            .returning(|| Ok(()));

        manager.set_sync_handler(Arc::new(mock_handler)).await;
        assert!(manager.resume().await.is_ok());
    }
}
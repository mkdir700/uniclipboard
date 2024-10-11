use anyhow::Result;
use log::{error, info};
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;

use crate::clipboard::LocalClipboardTrait;
use crate::key_mouse_monitor::KeyMouseMonitorTrait;
use crate::message::Payload;
use crate::remote_sync::RemoteSyncManagerTrait;

pub struct UniClipboard {
    clipboard: Arc<dyn LocalClipboardTrait>,
    remote_sync: Arc<dyn RemoteSyncManagerTrait>,
    key_mouse_monitor: Option<Arc<dyn KeyMouseMonitorTrait>>,
    is_running: Arc<RwLock<bool>>,
    is_paused: Arc<RwLock<bool>>,
    last_payload: Arc<RwLock<Option<Payload>>>,
}

impl UniClipboard {
    pub fn new(
        clipboard: Arc<dyn LocalClipboardTrait>,
        remote_sync: Arc<dyn RemoteSyncManagerTrait>,
        key_mouse_monitor: Option<Arc<dyn KeyMouseMonitorTrait>>,
    ) -> Self {
        Self {
            clipboard,
            remote_sync,
            key_mouse_monitor,
            is_running: Arc::new(RwLock::new(false)),
            is_paused: Arc::new(RwLock::new(false)),
            last_payload: Arc::new(RwLock::new(None)),
        }
    }

    #[cfg_attr(not(feature = "integration_tests"), ignore)]
    #[allow(dead_code)]
    pub fn get_clipboard(&self) -> Arc<dyn LocalClipboardTrait> {
        self.clipboard.clone()
    }

    #[cfg_attr(not(feature = "integration_tests"), ignore)]
    #[allow(dead_code)]
    pub fn get_remote_sync(&self) -> Arc<dyn RemoteSyncManagerTrait> {
        self.remote_sync.clone()
    }

    pub async fn start(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if *is_running {
            anyhow::bail!("Already running");
        }
        *is_running = true;

        // 启动本地剪切板监听
        let clipboard_receiver = self.clipboard.start_monitoring().await?;

        // 启动远程同步
        self.remote_sync.start().await?;

        // 启动本地到远程的同步任务
        self.start_local_to_remote_sync(clipboard_receiver).await?;

        self.start_remote_to_local_sync().await?;

        // 启动键盘鼠标监控
        if let Some(monitor) = self.key_mouse_monitor.as_ref() {
            monitor.start().await;
        }

        Ok(())
    }

    async fn start_local_to_remote_sync(
        &self,
        mut clipboard_receiver: mpsc::Receiver<Payload>,
    ) -> Result<()> {
        let remote_sync = self.remote_sync.clone();
        let is_running = self.is_running.clone();
        let last_payload = self.last_payload.clone();

        tokio::spawn(async move {
            while *is_running.read().await {
                if let Some(payload) = clipboard_receiver.recv().await {
                    let last_content = last_payload.read().await;
                    if let Some(last_payload) = last_content.as_ref() {
                        if last_payload.is_duplicate(&payload) {
                            info!(
                                "Skip push to remote: {}, because it's the same as the last one",
                                payload
                            );
                            continue;
                        }
                    }
                    let tmp = last_content.clone();
                    drop(last_content);

                    {
                        *last_payload.write().await = Some(payload.clone());
                    }

                    info!("Push to remote: {}", payload);
                    if let Err(e) = remote_sync.push(payload.clone()).await {
                        // 恢复到之前的值
                        *last_payload.write().await = tmp;
                        // 处理错误，可能需要重试或记录日志
                        error!("Failed to push to remote: {:?}", e);
                    }
                }
            }
        });

        Ok(())
    }

    async fn start_remote_to_local_sync(&self) -> Result<()> {
        let clipboard = self.clipboard.clone();
        let remote_sync = self.remote_sync.clone();
        let is_running = self.is_running.clone();
        let last_payload = self.last_payload.clone();

        tokio::spawn(async move {
            while *is_running.read().await {
                match remote_sync.pull(Some(Duration::from_secs(10))).await {
                    Ok(content) => {
                        info!("Set local clipboard: {}", content);
                        let tmp = last_payload.read().await.clone();
                        {
                            *last_payload.write().await = Some(content.clone());
                        }
                        if let Err(e) = clipboard.set_clipboard_content(content).await {
                            // 恢复到之前的值
                            *last_payload.write().await = tmp;
                            error!("Failed to set clipboard content: {:?}", e);
                        }
                    }
                    Err(e) => {
                        // 处理错误，可能需要重试或记录日志
                        error!("Failed to pull from remote: {:?}", e);
                    }
                }
                // 添加适当的延迟，避免过于频繁的同步
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        });

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut is_running = self.is_running.write().await;
        if !*is_running {
            anyhow::bail!("Not running");
        }
        *is_running = false;

        // 停止本地剪切板监听
        self.clipboard.stop_monitoring().await?;

        // 停止远程同步
        self.remote_sync.stop().await?;

        Ok(())
    }

    pub async fn pause(&self) -> Result<()> {
        let mut is_paused = self.is_paused.write().await;
        *is_paused = true;

        // 暂停本地剪切板监听
        self.clipboard.pause().await;

        // 暂停远程同步
        if let Err(e) = self.remote_sync.pause().await {
            error!("Failed to pause remote sync: {:?}", e);
        }

        Ok(())
    }

    pub async fn resume(&self) -> Result<()> {
        let mut is_paused = self.is_paused.write().await;
        *is_paused = false;

        // 恢复本地剪切板监听
        self.clipboard.resume().await;

        // 恢复远程同步
        if let Err(e) = self.remote_sync.resume().await {
            error!("Failed to resume remote sync: {:?}", e);
        }

        Ok(())
    }

    pub async fn wait_for_stop(&self) -> Result<()> {
        let mut last_is_sleep = false;

        loop {
            select! {
                _ = ctrl_c() => {
                    info!("收到 Ctrl+C，正在停止...");
                    break;
                }
                _ = async {
                    if let Some(monitor) = &self.key_mouse_monitor.as_ref() {
                        if monitor.is_sleep().await {
                            if !last_is_sleep {
                                if let Err(e) = self.pause().await {
                                    error!("暂停失败: {:?}", e);
                                }
                                last_is_sleep = true;
                                info!("剪贴板同步已暂停，因为键盘和鼠标处于睡眠状态");
                            }
                        } else {
                            if last_is_sleep {
                                if let Err(e) = self.resume().await {
                                    error!("恢复失败: {:?}", e);
                                }
                                last_is_sleep = false;
                                info!("剪贴板同步已恢复，因为键盘和鼠标已唤醒");
                            }
                        }
                    }
                    sleep(Duration::from_secs(1)).await;
                } => {}
            }
        }

        // 当收到 Ctrl+C 信号后，停止同步
        self.stop().await?;
        info!("剪贴板同步已停止");
        Ok(())
    }
}

pub struct UniClipboardBuilder {
    clipboard: Option<Arc<dyn LocalClipboardTrait>>,
    remote_sync: Option<Arc<dyn RemoteSyncManagerTrait>>,
    key_mouse_monitor: Option<Arc<dyn KeyMouseMonitorTrait>>,
}

impl UniClipboardBuilder {
    pub fn new() -> Self {
        Self {
            clipboard: None,
            remote_sync: None,
            key_mouse_monitor: None,
        }
    }

    pub fn set_local_clipboard(mut self, clipboard: Arc<dyn LocalClipboardTrait>) -> Self {
        self.clipboard = Some(clipboard);
        self
    }

    pub fn set_remote_sync(mut self, remote_sync: Arc<dyn RemoteSyncManagerTrait>) -> Self {
        self.remote_sync = Some(remote_sync);
        self
    }

    #[allow(dead_code)]
    pub fn set_key_mouse_monitor(
        mut self,
        key_mouse_monitor: Arc<dyn KeyMouseMonitorTrait>,
    ) -> Self {
        self.key_mouse_monitor = Some(key_mouse_monitor);
        self
    }

    pub fn build(self) -> Result<UniClipboard> {
        let clipboard = self
            .clipboard
            .ok_or_else(|| anyhow::anyhow!("No local clipboard set"))?;
        let remote_sync = self
            .remote_sync
            .ok_or_else(|| anyhow::anyhow!("No remote sync set"))?;

        Ok(UniClipboard::new(
            clipboard,
            remote_sync,
            self.key_mouse_monitor,
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::remote_sync::RemoteClipboardSync;

    use super::*;
    use anyhow::Result;
    use async_trait::async_trait;
    use bytes::Bytes;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // 模拟本地剪贴板
    struct MockLocalClipboard {
        content: Arc<Mutex<Option<Payload>>>,
    }

    #[async_trait::async_trait]
    impl LocalClipboardTrait for MockLocalClipboard {
        async fn start_monitoring(&self) -> Result<tokio::sync::mpsc::Receiver<Payload>> {
            // 简化实现，返回一个空的接收器
            let (_, rx) = tokio::sync::mpsc::channel(10);
            Ok(rx)
        }

        async fn read(&self) -> Result<Payload> {
            Ok(self.content.lock().await.clone().unwrap())
        }

        async fn write(&self, content: Payload) -> Result<()> {
            *self.content.lock().await = Some(content);
            Ok(())
        }

        async fn stop_monitoring(&self) -> Result<()> {
            Ok(())
        }

        async fn set_clipboard_content(&self, content: Payload) -> Result<()> {
            *self.content.lock().await = Some(content);
            Ok(())
        }

        async fn pause(&self) {}

        async fn resume(&self) {}
    }

    // 模拟远程同步管理器
    struct MockRemoteSync {
        content: Arc<Mutex<Option<Payload>>>,
    }

    #[async_trait]
    impl RemoteSyncManagerTrait for MockRemoteSync {
        async fn sync(&self) -> Result<()> {
            Ok(())
        }

        async fn set_sync_handler(&self, _handler: Arc<dyn RemoteClipboardSync>) {}

        async fn start(&self) -> Result<()> {
            Ok(())
        }

        async fn stop(&self) -> Result<()> {
            Ok(())
        }

        async fn push(&self, payload: Payload) -> Result<()> {
            *self.content.lock().await = Some(payload);
            Ok(())
        }

        async fn pull(&self, _timeout: Option<std::time::Duration>) -> Result<Payload> {
            Ok(self.content.lock().await.clone().unwrap())
        }

        async fn pause(&self) -> Result<()> {
            Ok(())
        }

        async fn resume(&self) -> Result<()> {
            Ok(())
        }
    }

    // 模拟键鼠监控器
    struct MockKeyMouseMonitor {
        is_sleep: Arc<Mutex<bool>>,
    }

    #[async_trait::async_trait]
    impl KeyMouseMonitorTrait for MockKeyMouseMonitor {
        async fn start(&self) {}

        async fn is_sleep(&self) -> bool {
            *self.is_sleep.lock().await
        }
    }

    #[tokio::test]
    async fn test_uni_clipboard_creation() {
        let clipboard = Arc::new(MockLocalClipboard {
            content: Arc::new(Mutex::new(None)),
        });
        let remote_sync = Arc::new(MockRemoteSync {
            content: Arc::new(Mutex::new(None)),
        });
        let key_mouse_monitor = Arc::new(MockKeyMouseMonitor {
            is_sleep: Arc::new(Mutex::new(false)),
        });

        let uni_clipboard = UniClipboardBuilder::new()
            .set_local_clipboard(clipboard)
            .set_remote_sync(remote_sync)
            .set_key_mouse_monitor(key_mouse_monitor)
            .build()
            .expect("Failed to build UniClipboard");

        assert!(uni_clipboard.start().await.is_ok());
        assert!(uni_clipboard.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_uni_clipboard_sync() {
        let clipboard = Arc::new(MockLocalClipboard {
            content: Arc::new(Mutex::new(None)),
        });
        let remote_sync = Arc::new(MockRemoteSync {
            content: Arc::new(Mutex::new(None)),
        });

        let uni_clipboard = UniClipboardBuilder::new()
            .set_local_clipboard(clipboard.clone())
            .set_remote_sync(remote_sync.clone())
            .build()
            .expect("Failed to build UniClipboard");

        assert!(uni_clipboard.start().await.is_ok());

        // 模拟远程同步
        let test_payload = Payload::new_text(
            Bytes::from("Test content".to_string()),
            "device_id".to_string(),
            chrono::Utc::now(),
        );
        assert!(remote_sync.push(test_payload.clone()).await.is_ok());

        // 等待同步
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // 检查本地剪贴板是否已更新
        let local_content = clipboard.content.lock().await;
        assert_eq!(*local_content, Some(test_payload));

        assert!(uni_clipboard.stop().await.is_ok());
    }

    #[tokio::test]
    async fn test_uni_clipboard_pause_resume() {
        let clipboard = Arc::new(MockLocalClipboard {
            content: Arc::new(Mutex::new(None)),
        });
        let remote_sync = Arc::new(MockRemoteSync {
            content: Arc::new(Mutex::new(None)),
        });

        let uni_clipboard = UniClipboardBuilder::new()
            .set_local_clipboard(clipboard)
            .set_remote_sync(remote_sync)
            .build()
            .expect("Failed to build UniClipboard");

        assert!(uni_clipboard.start().await.is_ok());
        assert!(uni_clipboard.pause().await.is_ok());
        assert!(uni_clipboard.resume().await.is_ok());
        assert!(uni_clipboard.stop().await.is_ok());
    }
}

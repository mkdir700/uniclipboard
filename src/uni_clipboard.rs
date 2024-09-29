use anyhow::Result;
use log::{error, info};
use std::sync::Arc;
use std::time::Duration;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::sync::{mpsc, RwLock};
use tokio::time::sleep;

use crate::clipboard_handler::LocalClipboard;
use crate::key_mouse_monitor::KeyMouseMonitor;
use crate::message::Payload;
use crate::remote_sync::manager::RemoteSyncManager;
use crate::remote_sync::{RemoteClipboardSync, WebDavSync, WebSocketSync};
use crate::WebDAVClient;

pub struct UniClipboard {
    clipboard: Arc<LocalClipboard>,
    remote_sync: Arc<RemoteSyncManager>,
    key_mouse_monitor: Arc<Option<KeyMouseMonitor>>,
    is_running: Arc<RwLock<bool>>,
    is_paused: Arc<RwLock<bool>>,
    last_content_hash: Arc<RwLock<Option<String>>>,
}

impl UniClipboard {
    pub fn new(
        clipboard: LocalClipboard,
        remote_sync: RemoteSyncManager,
        key_mouse_monitor: Option<KeyMouseMonitor>,
    ) -> Self {
        Self {
            clipboard: Arc::new(clipboard),
            remote_sync: Arc::new(remote_sync),
            key_mouse_monitor: Arc::new(key_mouse_monitor),
            is_running: Arc::new(RwLock::new(false)),
            is_paused: Arc::new(RwLock::new(false)),
            last_content_hash: Arc::new(RwLock::new(None)),
        }
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
        let last_content_hash = self.last_content_hash.clone();

        tokio::spawn(async move {
            while *is_running.read().await {
                if let Some(payload) = clipboard_receiver.recv().await {
                    if *last_content_hash
                        .read()
                        .await
                        .clone()
                        .unwrap_or(String::from(""))
                        == payload.get_key()
                    {
                        info!(
                            "Skip push to remote: {}, because it's the same as the last one",
                            payload
                        );
                        continue;
                    }

                    info!("Push to remote: {}", payload);
                    if let Err(e) = remote_sync.push(payload).await {
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
        let last_content_hash = self.last_content_hash.clone();

        tokio::spawn(async move {
            while *is_running.read().await {
                match remote_sync.pull(Some(Duration::from_secs(10))).await {
                    Ok(content) => {
                        info!("Set local clipboard: {}", content);
                        let content_hash = content.get_key();
                        if let Err(e) = clipboard.set_clipboard_content(content).await {
                            error!("Failed to set clipboard content: {:?}", e);
                        }
                        *last_content_hash.write().await = Some(content_hash);
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
    clipboard: Option<LocalClipboard>,
    remote_sync: Option<Arc<dyn RemoteClipboardSync>>,
    key_mouse_monitor: Option<KeyMouseMonitor>,
}

impl UniClipboardBuilder {
    pub fn new() -> Self {
        Self {
            clipboard: None,
            remote_sync: None,
            key_mouse_monitor: None,
        }
    }

    #[allow(dead_code)]
    pub fn set_key_mouse_monitor(mut self, key_mouse_monitor: KeyMouseMonitor) -> Self {
        self.key_mouse_monitor = Some(key_mouse_monitor);
        self
    }

    pub fn set_local_clipboard(mut self, clipboard: LocalClipboard) -> Self {
        self.clipboard = Some(clipboard);
        self
    }

    pub fn set_websocket_sync(mut self, is_server: bool) -> Self {
        self.remote_sync = Some(Arc::new(WebSocketSync::new(is_server)));
        self
    }

    #[allow(dead_code)]
    pub fn set_webdav_sync(mut self, webdav_client: WebDAVClient) -> Self {
        self.remote_sync = Some(Arc::new(WebDavSync::new(webdav_client)));
        self
    }

    pub async fn build(self) -> Result<UniClipboard> {
        let clipboard = if let Some(clipboard) = self.clipboard {
            clipboard
        } else {
            return Err(anyhow::anyhow!("No local clipboard enabled"));
        };
        let remote_sync = if let Some(remote_sync) = self.remote_sync {
            remote_sync
        } else {
            return Err(anyhow::anyhow!("No remote sync enabled"));
        };
        let remote_sync_manager = RemoteSyncManager::new();
        remote_sync_manager.set_sync_handler(remote_sync).await;

        Ok(UniClipboard::new(
            clipboard,
            remote_sync_manager,
            self.key_mouse_monitor,
        ))
    }
}

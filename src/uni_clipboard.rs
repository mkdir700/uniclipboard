use anyhow::Result;
use log::{error, info};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};

use crate::clipboard_handler::LocalClipboard;
use crate::message::Payload;
use crate::remote_sync::manager::RemoteSyncManager;

pub struct UniClipboard {
    clipboard: Arc<LocalClipboard>,
    remote_sync: Arc<RemoteSyncManager>,
    is_running: Arc<RwLock<bool>>,
    last_content_hash: Arc<RwLock<Option<String>>>,
}

impl UniClipboard {
    pub fn new(clipboard: LocalClipboard, remote_sync: RemoteSyncManager) -> Self {
        Self {
            clipboard: Arc::new(clipboard),
            remote_sync: Arc::new(remote_sync),
            is_running: Arc::new(RwLock::new(false)),
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
                        == payload.hash()
                    {
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
                        let content_hash = content.hash();
                        if let Err(e) = clipboard.set_clipboard_content(content).await {
                            error!("Failed to set clipboard content: {:?}", e);
                        }
                        info!("Set local clipboard: {}", content_hash.clone());
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

    #[allow(dead_code)]
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

    pub async fn wait_for_stop(&self) -> Result<()> {
        let is_running = self.is_running.clone();
        while *is_running.read().await {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        Ok(())
    }

    // pub async fn start(&self) -> Result<(), Box<dyn Error>> {
    //     let mut last_is_sleep = false;
    //     match &self.key_mouse_monitor {
    //         Some(monitor) => monitor.start().await,
    //         None => (),
    //     }

    //     // 启动剪贴板监控
    //     self.clipboard_handler.start().await?;

    //     // 如果是 server 端，则启动 websocket 服务
    //     // 如果是 client 端，则连接 server 端
    //     if self.is_server {
    //         self.start_websocket_server().await?;
    //     } else {
    //         self.connect_server().await?;
    //     }

    //     // 监控系统是否处于休眠状态，并根据状态暂停或恢复剪贴板同步。
    //     loop {
    //         select! {
    //             _ = ctrl_c() => {
    //                 info!("Received Ctrl+C, stopping...");
    //                 break;
    //             }
    //             _ = async {
    //                 if let Some(monitor) = &self.key_mouse_monitor {
    //                     if monitor.is_sleep().await {
    //                         if !last_is_sleep {
    //                             self.clipboard_handler.pause().await;
    //                             last_is_sleep = true;
    //                             info!("Keyboard and mouse is sleeping, pausing clipboard sync");
    //                         }
    //                     } else {
    //                         if last_is_sleep {
    //                             self.clipboard_handler.resume().await;
    //                             last_is_sleep = false;
    //                             info!("Keyboard and mouse is awake, resuming clipboard sync");
    //                         }
    //                     }
    //                 }
    //                 sleep(Duration::from_millis(100)).await;
    //             } => {}
    //         }
    //     }

    //     match &self.key_mouse_monitor {
    //         Some(monitor) => monitor.stop().await,
    //         None => (),
    //     }

    //     Ok(())
    // }
}

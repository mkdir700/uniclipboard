use crate::clipboard::create_clipboard;
use crate::clipboard::traits::ClipboardOperations;
use crate::message::Payload;
use anyhow::Result;
use log::debug;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{sleep, Duration, Instant};

pub struct LocalClipboard {
    last_content_hash: Arc<TokioMutex<Option<String>>>,
    factory: Arc<dyn ClipboardOperations>,
    paused: Arc<TokioMutex<bool>>,
    stopped: Arc<TokioMutex<bool>>,
}

impl LocalClipboard {
    pub fn new() -> Self {
        let factory = create_clipboard().unwrap();
        Self {
            last_content_hash: Arc::new(TokioMutex::new(None)),
            factory,
            paused: Arc::new(TokioMutex::new(false)),
            stopped: Arc::new(TokioMutex::new(false)),
        }
    }

    pub async fn pause(&self) {
        let mut is_paused = self.paused.lock().await;
        *is_paused = true;
    }

    pub async fn resume(&self) {
        let mut is_paused = self.paused.lock().await;
        *is_paused = false;
    }

    pub async fn read(&self) -> Result<Payload> {
        self.factory.read()
    }

    pub async fn write(&self, payload: Payload) -> Result<()> {
        self.factory.write(payload)
    }

    pub async fn pull(&self, timeout: Option<Duration>) -> Result<Payload> {
        let start_time = Instant::now();

        loop {
            if *self.paused.lock().await {
                sleep(Duration::from_millis(200)).await;
                continue;
            }

            let current = self.read().await?;
            let current_hash = current.hash();

            // 使用 RwLock 来安全地访问和修改 last_content_hash
            {
                let last_hash = self.last_content_hash.lock().await;
                if last_hash.as_ref() != Some(&current_hash) {
                    // 如果哈希值不同，更新 last_content_hash 并返回当前内容
                    drop(last_hash); // 释放读锁
                    let mut last_hash = self.last_content_hash.lock().await;
                    *last_hash = Some(current_hash);
                    return Ok(current);
                }
            }

            // 检查是否超时
            if let Some(timeout) = timeout {
                if start_time.elapsed() > timeout {
                    return Err(anyhow::anyhow!("拉取操作超时").into());
                }
            }

            sleep(Duration::from_millis(200)).await;
        }
    }

    pub async fn start_monitoring(self: &Arc<Self>) -> Result<mpsc::Receiver<Payload>> {
        let (tx, rx) = mpsc::channel(100);
        let clipboard = Arc::clone(self);

        tokio::spawn(async move {
            let mut last_content_hash: Option<String> = None;
            loop {
                if let Ok(payload) = clipboard.pull(None).await {
                    // 判断是否为重复的内容
                    let current_content_hash = payload.hash();
                    if let Some(ref hash) = last_content_hash {
                        if hash == &current_content_hash {
                            continue;
                        }
                    }
                    debug!("Watch new content from local: {}", payload);
                    tx.send(payload).await.unwrap();
                    last_content_hash = Some(current_content_hash);
                }
            }
        });
        Ok(rx)
    }

    pub async fn stop_monitoring(&self) -> Result<()> {
        let mut is_stopped = self.stopped.lock().await;
        *is_stopped = true;
        Ok(())
    }

    pub async fn set_clipboard_content(&self, content: Payload) -> Result<()> {
        self.write(content).await
    }
}

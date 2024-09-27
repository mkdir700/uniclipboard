use crate::clipboard::{RsClipboard, RsClipboardChangeHandler};
use crate::message::Payload;
use anyhow::Result;
use clipboard_rs::WatcherShutdown;
use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
use log::debug;
use log::error;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::{mpsc, Notify};
use tokio::time::{sleep, Duration, Instant};

pub struct LocalClipboard {
    last_content_hash: Arc<TokioMutex<Option<String>>>,
    rs_clipboard: Arc<RsClipboard>,
    paused: Arc<TokioMutex<bool>>,
    stopped: Arc<TokioMutex<bool>>,
    watcher: Arc<Mutex<ClipboardWatcherContext<RsClipboardChangeHandler>>>,
    watcher_shutdown: Arc<TokioMutex<Option<WatcherShutdown>>>,
}

impl LocalClipboard {
    pub fn new() -> Self {
        let notify = Arc::new(Notify::new());
        let rs_clipboard = RsClipboard::new(notify.clone()).unwrap();
        let clipboard_change_handler = RsClipboardChangeHandler::new(notify);
        let mut watcher: ClipboardWatcherContext<RsClipboardChangeHandler> =
            ClipboardWatcherContext::new().unwrap();
        let watcher_shutdown = watcher
            .add_handler(clipboard_change_handler)
            .get_shutdown_channel();
        Self {
            last_content_hash: Arc::new(TokioMutex::new(None)),
            rs_clipboard: Arc::new(rs_clipboard),
            paused: Arc::new(TokioMutex::new(false)),
            stopped: Arc::new(TokioMutex::new(false)),
            watcher: Arc::new(Mutex::new(watcher)),
            watcher_shutdown: Arc::new(TokioMutex::new(Some(watcher_shutdown))),
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
        self.rs_clipboard.read()
    }

    pub async fn write(&self, payload: Payload) -> Result<()> {
        self.rs_clipboard.write(payload)
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
        let c = Arc::clone(self);
        let rs_clipboard = Arc::clone(&c.rs_clipboard);
        let watcher = Arc::clone(&c.watcher);

        // 在后台线程中启动剪贴板监听
        thread::spawn(move || {
            let mut watcher = watcher.lock().expect("Failed to lock watcher");
            debug!("Start watching clipboard");
            watcher.start_watch();
        });

        // 在异步任务中处理剪贴板变化
        tokio::spawn(async move {
            loop {
                if *c.stopped.lock().await {
                    break;
                }

                if let Err(e) = rs_clipboard.wait_clipboard_change().await {
                    error!("Wait clipboard change failed: {:?}", e);
                    continue;
                }

                if let Ok(payload) = c.pull(None).await {
                    debug!("Wait clipboard change: {}", payload);
                    if let Err(e) = tx.send(payload).await {
                        error!("Send payload failed: {:?}", e);
                    }
                }
            }
        });
        Ok(rx)
    }

    pub async fn stop_monitoring(&self) -> Result<()> {
        let mut is_stopped = self.stopped.lock().await;
        *is_stopped = true;
        if let Some(shutdown) = self.watcher_shutdown.lock().await.take() {
            shutdown.stop();
        }
        Ok(())
    }

    pub async fn set_clipboard_content(&self, content: Payload) -> Result<()> {
        self.write(content).await
    }
}

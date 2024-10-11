use super::traits::LocalClipboardTrait;
use crate::clipboard::{RsClipboard, RsClipboardChangeHandler};
use crate::message::Payload;
use anyhow::Result;
use async_trait::async_trait;
use clipboard_rs::WatcherShutdown;
use clipboard_rs::{ClipboardWatcher, ClipboardWatcherContext};
use log::debug;
use log::error;
use std::sync::{Arc, Mutex};
use std::thread;
use tokio::sync::{mpsc, Notify};
use tokio::sync::{Mutex as TokioMutex, RwLock};

#[derive(Clone)]
pub struct LocalClipboard {
    rs_clipboard: Arc<RsClipboard>,
    paused: Arc<(TokioMutex<bool>, Notify)>,
    stopped: Arc<TokioMutex<bool>>,
    watcher: Arc<Mutex<ClipboardWatcherContext<RsClipboardChangeHandler>>>,
    watcher_shutdown: Arc<TokioMutex<Option<WatcherShutdown>>>,
    rw_lock: Arc<RwLock<bool>>,
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
            rs_clipboard: Arc::new(rs_clipboard),
            paused: Arc::new((TokioMutex::new(false), Notify::new())),
            stopped: Arc::new(TokioMutex::new(false)),
            watcher: Arc::new(Mutex::new(watcher)),
            watcher_shutdown: Arc::new(TokioMutex::new(Some(watcher_shutdown))),
            rw_lock: Arc::new(RwLock::new(false)),
        }
    }
}

#[async_trait]
impl LocalClipboardTrait for LocalClipboard {
    async fn pause(&self) {
        let mut is_paused = self.paused.0.lock().await;
        *is_paused = true;
    }

    async fn resume(&self) {
        let mut is_paused = self.paused.0.lock().await;
        *is_paused = false;
        self.paused.1.notify_waiters();
    }

    async fn read(&self) -> Result<Payload> {
        let reading_lock = self.rw_lock.read().await;
        let payload = self.rs_clipboard.read()?;
        drop(reading_lock);
        Ok(payload)
    }

    async fn write(&self, payload: Payload) -> Result<()> {
        let writing_lock = self.rw_lock.write().await;
        self.rs_clipboard.write(payload)?;
        drop(writing_lock);
        Ok(())
    }

    async fn start_monitoring(&self) -> Result<mpsc::Receiver<Payload>> {
        let (tx, rx) = mpsc::channel(100);
        let rs_clipboard = Arc::clone(&self.rs_clipboard);
        let watcher = Arc::clone(&self.watcher);
        let stopped = Arc::clone(&self.stopped);
        let paused = Arc::clone(&self.paused);
        let self_clone = self.clone();

        // 在后台线程中启动剪贴板监听
        thread::spawn(move || {
            let mut watcher = watcher.lock().expect("Failed to lock watcher");
            debug!("Start watching clipboard");
            watcher.start_watch();
        });

        // 在异步任务中处理剪贴板变化
        tokio::spawn(async move {
            loop {
                if *stopped.lock().await {
                    break;
                }

                if *paused.0.lock().await {
                    paused.1.notified().await;
                    continue;
                }

                if let Err(e) = rs_clipboard.wait_clipboard_change().await {
                    error!("Wait clipboard change failed: {:?}", e);
                    continue;
                }

                if *paused.0.lock().await {
                    paused.1.notified().await;
                    continue;
                }

                match self_clone.read().await {
                    Ok(payload) => {
                        debug!("Wait clipboard change: {}", payload);
                        if let Err(e) = tx.send(payload).await {
                            error!("Send payload failed: {:?}", e);
                        }
                    }
                    Err(e) => {
                        error!("Read clipboard failed: {:?}", e);
                    }
                }
            }
        });
        Ok(rx)
    }

    async fn stop_monitoring(&self) -> Result<()> {
        let mut is_stopped = self.stopped.lock().await;
        *is_stopped = true;
        if let Some(shutdown) = self.watcher_shutdown.lock().await.take() {
            shutdown.stop();
        }
        Ok(())
    }

    async fn set_clipboard_content(&self, content: Payload) -> Result<()> {
        self.write(content).await
    }
}

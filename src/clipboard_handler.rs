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

pub struct LocalClipboard {
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

                match c.read().await {
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

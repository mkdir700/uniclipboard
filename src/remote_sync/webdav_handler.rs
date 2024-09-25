use super::traits::RemoteClipboardSync;
use crate::config::CONFIG;
use crate::{message::Payload, network::WebDAVClient};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::Mutex as TokioMutex;
use tokio::time::{sleep, Duration, Instant};

#[derive(Clone)]
pub struct WebDavSync {
    client: Arc<WebDAVClient>,
    last_modified: Arc<RwLock<Option<DateTime<Utc>>>>,
    pub base_path: String,
    // 是否在程序启动后，立即从云端拉取最近的一个内容
    #[allow(dead_code)]
    pull_on_start: bool,
    // 是否暂停从云端拉取
    paused: Arc<TokioMutex<bool>>,
    running: Arc<TokioMutex<bool>>,
}

impl WebDavSync {
    pub fn new(client: WebDAVClient) -> Self {
        let base_path = format!("/uniclipboard");
        Self {
            client: Arc::new(client),
            base_path,
            last_modified: Arc::new(RwLock::new(None)),
            pull_on_start: true,
            paused: Arc::new(TokioMutex::new(false)),
            running: Arc::new(TokioMutex::new(false)),
        }
    }

    /// The function `get_client` returns a cloned reference to the `WebDAVClient` wrapped in an `Arc`.
    ///
    /// Returns:
    ///
    /// An `Arc` (atomic reference counted) smart pointer to a `WebDAVClient` client is being returned.
    #[allow(dead_code)]
    pub fn get_client(&self) -> Arc<WebDAVClient> {
        Arc::clone(&self.client)
    }
}

#[async_trait]
impl RemoteClipboardSync for WebDavSync {
    async fn sync(&self) -> Result<()> {
        todo!()
    }
    async fn start(&self) -> Result<()> {
        let mut is_running = self.running.lock().await;
        if *is_running {
            anyhow::bail!("Already running");
        }
        *is_running = true;
        Ok(())
    }
    async fn stop(&self) -> Result<()> {
        let mut is_running = self.running.lock().await;
        *is_running = false;
        Ok(())
    }
    async fn pause(&self) -> Result<()> {
        let mut is_pause_pull = self.paused.lock().await;
        *is_pause_pull = true;
        Ok(())
    }

    async fn resume(&self) -> Result<()> {
        let mut is_pause_pull = self.paused.lock().await;
        *is_pause_pull = false;
        Ok(())
    }

    /// Pushes new content to the cloud clipboard.
    ///
    /// This method uploads the given content to the WebDAV server using the
    /// configured base path and share code.
    ///
    /// # Arguments
    ///
    /// * `content` - A String containing the content to be uploaded.
    ///
    /// # Returns
    ///
    /// Returns the path of the uploaded file.
    ///
    /// # Errors
    ///
    /// This function will return an error if the upload to the WebDAV server fails.
    async fn push(&self, payload: Payload) -> Result<()> {
        let _path = self.client.upload(self.base_path.clone(), payload).await?;
        Ok(())
    }

    /// Pulls the latest content from the cloud clipboard.
    ///
    /// This method continuously checks for new files added to the WebDAV server
    /// at the specified base path. When a new file is detected (i.e., a file with
    /// a modification time later than the latest known file), it downloads and
    /// returns the content of that file as a Payload.
    ///
    /// # Returns
    ///
    /// Returns a `Result` which is `Ok(Payload)` if a new file is successfully
    /// retrieved and parsed, or an `Error` if any operation fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - There's a failure in communicating with the WebDAV server
    /// - The latest file cannot be retrieved or parsed into a Payload
    async fn pull(&self, timeout: Option<Duration>) -> Result<Payload> {
        // FIXME: 当前的逻辑，如果是在程序首次启动后，就会从云端拉取最新的
        // 应该给出选项，在程序启动后，是否立即从云端拉取最近的一个内容
        let start_time = Instant::now();
        let mut latest_file_meta;

        loop {
            if *self.paused.lock().await {
                sleep(Duration::from_millis(200)).await;
                continue;
            }

            if !*self.running.lock().await {
                return Err(anyhow::anyhow!("Not running"));
            }

            if let Some(timeout) = timeout {
                if start_time.elapsed() > timeout {
                    anyhow::bail!("Timeout while waiting for clipboard change");
                }
            }

            latest_file_meta = self
                .client
                .fetch_latest_file_meta(self.base_path.clone())
                .await?;

            let file_path = latest_file_meta.get_path();
            let modified = latest_file_meta.last_modified;
            let device_id = latest_file_meta.get_device_id();
            // 如果设备 id 相同,则跳过
            if device_id == CONFIG.read().unwrap().get_device_id() {
                // 休眠 200ms
                sleep(std::time::Duration::from_millis(200)).await;
                continue;
            }

            let should_update = {
                let last_modified = self.last_modified.read().unwrap();
                match last_modified.as_ref() {
                    None => true,
                    Some(last_modified) => modified > *last_modified,
                }
            };
            if should_update {
                let payload = self.client.download(file_path).await?;
                {
                    let mut last_modified = self.last_modified.write().unwrap();
                    *last_modified = Some(modified);
                }
                return Ok(payload);
            }

            // 休眠 200ms
            sleep(std::time::Duration::from_millis(200)).await;
        }
    }
}

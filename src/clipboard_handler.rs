use crate::clipboard::create_clipboard;
use crate::clipboard::traits::ClipboardOperations;
use crate::config::CONFIG;
use crate::{message::Payload, network::WebDAVClient};
use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{debug, error, info};
use std::sync::RwLock;
use std::{
    collections::VecDeque,
    error::Error,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex as TokioMutex;
use tokio::task;

#[derive(Clone)]
pub struct CloudClipboardHandler {
    client: Arc<WebDAVClient>,
    last_modified: Arc<RwLock<Option<DateTime<Utc>>>>,
    pub base_path: String,
    // 是否在程序启动后，立即从云端拉取最近的一个内容
    #[allow(dead_code)]
    pull_on_start: bool,
}

pub struct LocalClipboardHandler {
    factory: Arc<dyn ClipboardOperations>,
}

pub struct ClipboardHandler {
    last_content_hash: Arc<RwLock<Option<String>>>,
    cloud: Arc<CloudClipboardHandler>,
    local: Arc<LocalClipboardHandler>,
    cloud_to_local_queue: Arc<TokioMutex<VecDeque<Payload>>>,
    local_to_cloud_queue: Arc<TokioMutex<VecDeque<Payload>>>,
}

impl CloudClipboardHandler {
    pub fn new(client: WebDAVClient) -> Self {
        let base_path = format!("/uniclipboard");
        Self {
            client: Arc::new(client),
            base_path,
            last_modified: Arc::new(RwLock::new(None)),
            pull_on_start: true,
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
    pub async fn push(&self, payload: Payload) -> Result<String, Box<dyn Error>> {
        let path = self.client.upload(self.base_path.clone(), payload).await?;
        Ok(path)
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
    pub async fn pull(&self, timeout: Option<Duration>) -> Result<Payload> {
        // FIXME: 当前的逻辑，如果是在程序首次启动后，就会从云端拉取最新的
        // 应该给出选项，在程序启动后，是否立即从云端拉取最近的一个内容
        let start_time = Instant::now();
        let mut latest_file_meta;

        loop {
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
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
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
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }
}

impl LocalClipboardHandler {
    pub fn new() -> Self {
        let factory = create_clipboard().unwrap();
        Self { factory }
    }

    pub async fn read(&self) -> Result<Payload> {
        self.factory.read()
    }

    pub async fn write(&self, payload: Payload) -> Result<()> {
        self.factory.write(payload)
    }

    pub async fn pull(&self, timeout: Option<Duration>) -> Result<Payload> {
        let latest = self.read().await?;
        let start_time = std::time::Instant::now();

        loop {
            let current = self.read().await?;
            if !current.eq(&latest) {
                return Ok(current);
            }

            if let Some(timeout) = timeout {
                if start_time.elapsed() > timeout {
                    anyhow::bail!("Timeout while waiting for clipboard change");
                }
            }

            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
}

impl ClipboardHandler {
    pub fn new(
        cloud_clipboard_handler: CloudClipboardHandler,
        local_clipboard_handler: LocalClipboardHandler,
    ) -> Self {
        Self {
            last_content_hash: Arc::new(RwLock::new(None)),
            cloud: Arc::new(cloud_clipboard_handler),
            local: Arc::new(local_clipboard_handler),
            cloud_to_local_queue: Arc::new(TokioMutex::new(VecDeque::new())),
            local_to_cloud_queue: Arc::new(TokioMutex::new(VecDeque::new())),
        }
    }

    pub async fn watch(&self) -> Result<(), Box<dyn Error>> {
        let cloud_watcher = self.watch_cloud_clipboard();
        let local_watcher = self.watch_local_clipboard();
        let cloud_to_local_handler = self.cloud_to_local_task();
        let local_to_cloud_handler = self.local_to_cloud_task();

        tokio::try_join!(
            cloud_watcher,
            local_watcher,
            cloud_to_local_handler,
            local_to_cloud_handler
        )?;

        Ok(())
    }

    async fn watch_cloud_clipboard(&self) -> Result<(), Box<dyn Error>> {
        let cloud = self.cloud.clone();
        let queue = Arc::clone(&self.cloud_to_local_queue);

        task::spawn(async move {
            loop {
                if let Ok(content) = cloud.pull(Some(Duration::from_secs(1))).await {
                    debug!("Watch new content from cloud: {}", content);
                    let mut queue = queue.lock().await;
                    queue.push_back(content);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await?;

        Ok(())
    }

    async fn watch_local_clipboard(&self) -> Result<(), Box<dyn Error>> {
        let local = Arc::clone(&self.local);
        let queue = Arc::clone(&self.local_to_cloud_queue);

        task::spawn(async move {
            loop {
                if let Ok(payload) = local.pull(Some(Duration::from_secs(1))).await {
                    debug!("Watch new content from local: {}", payload);
                    let mut queue = queue.lock().await;
                    queue.push_back(payload);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await?;

        Ok(())
    }

    async fn cloud_to_local_task(&self) -> Result<(), Box<dyn Error>> {
        let local = Arc::clone(&self.local);
        let queue = Arc::clone(&self.cloud_to_local_queue);
        let last_content_hash: Arc<RwLock<Option<String>>> = Arc::clone(&self.last_content_hash);

        task::spawn(async move {
            loop {
                let payload = {
                    let mut queue = queue.lock().await;
                    queue.pop_front()
                };

                if let Some(p) = payload {
                    info!("Push to local: {}", p);
                    let content_hash = p.hash();
                    if let Err(e) = local.write(p.clone()).await {
                        error!("Failed to write to local clipboard: {}", e);
                    } else {
                        info!("Write to local success: {}", p);
                        // 写入成功之后，记录 content_hash
                        *last_content_hash.write().unwrap() = Some(content_hash);
                    }
                } 
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await?;

        Ok(())
    }

    async fn local_to_cloud_task(&self) -> Result<(), Box<dyn Error>> {
        let cloud = self.cloud.clone();
        let queue = Arc::clone(&self.local_to_cloud_queue);
        let content_hash = Arc::clone(&self.last_content_hash);

        task::spawn(async move {
            loop {
                let payload = {
                    let mut queue = queue.lock().await;
                    queue.pop_front()
                };

                if let Some(p) = payload {
                    let new_hash = p.hash();
                    let should_upload = {
                        let last_content_hash = content_hash.read().unwrap();
                        last_content_hash.as_ref() != Some(&new_hash)
                    };

                    if !should_upload {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        info!("Skip upload to cloud: {}", p);
                        continue;
                    }
                    
                    info!("Push to cloud: {}", p);
                    if let Err(e) = cloud.push(p.clone()).await {
                        error!("Failed to push to cloud: {}", e);
                    } else {
                        info!("Upload to cloud success: {}", p);
                        // 更新成功后，更新 last_content_hash
                        *content_hash.write().unwrap() = Some(new_hash);
                    }
                } 
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await?;

        Ok(())
    }
}

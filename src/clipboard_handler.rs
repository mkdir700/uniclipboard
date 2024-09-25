use crate::clipboard::create_clipboard;
use crate::clipboard::traits::ClipboardOperations;
use crate::config::CONFIG;
use crate::{message::Payload, network::WebDAVClient};
use anyhow::Result;
use chrono::{DateTime, Utc};
use log::{debug, error, info};
use std::sync::RwLock;
use std::{collections::VecDeque, error::Error, sync::Arc};
use tokio::sync::Mutex as TokioMutex;
use tokio::task;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration, Instant};

#[derive(Clone)]
pub struct CloudClipboardHandler {
    client: Arc<WebDAVClient>,
    last_modified: Arc<RwLock<Option<DateTime<Utc>>>>,
    pub base_path: String,
    // 是否在程序启动后，立即从云端拉取最近的一个内容
    #[allow(dead_code)]
    pull_on_start: bool,
    // 是否暂停从云端拉取
    paused: Arc<TokioMutex<bool>>,
}

pub struct LocalClipboardHandler {
    last_content_hash: Arc<TokioMutex<Option<String>>>,
    factory: Arc<dyn ClipboardOperations>,
    paused: Arc<TokioMutex<bool>>,
}

pub struct ClipboardHandler {
    last_content_hash: Arc<RwLock<Option<String>>>,
    cloud: Arc<CloudClipboardHandler>,
    local: Arc<LocalClipboardHandler>,
    cloud_to_local_queue: Arc<TokioMutex<VecDeque<Payload>>>,
    local_to_cloud_queue: Arc<TokioMutex<VecDeque<Payload>>>,
    tasks: Arc<RwLock<Vec<JoinHandle<()>>>>,
}

impl CloudClipboardHandler {
    pub fn new(client: WebDAVClient) -> Self {
        let base_path = format!("/uniclipboard");
        Self {
            client: Arc::new(client),
            base_path,
            last_modified: Arc::new(RwLock::new(None)),
            pull_on_start: true,
            paused: Arc::new(TokioMutex::new(false)),
        }
    }

    pub async fn pause(&self) {
        let mut is_pause_pull = self.paused.lock().await;
        *is_pause_pull = true;
    }

    pub async fn resume(&self) {
        let mut is_pause_pull = self.paused.lock().await;
        *is_pause_pull = false;
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
    /// configured base path.
    ///
    /// it will delete the oldest file if the number of files exceeds the max_history
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
        // 删除旧的文件
        let max_history = CONFIG.read().unwrap().max_history;
        if let Some(max_history) = max_history {
            let count = self.client.count_files(self.base_path.clone()).await?;
            if count > max_history as usize {
                let oldest_file = self
                    .client
                    .fetch_oldest_file_meta(self.base_path.clone())
                    .await?;
                self.client.delete(oldest_file.get_path()).await?;
                debug!(
                    "Delete oldest file, path: {}, count: {}",
                    oldest_file.get_path(),
                    count
                );
            }
        }
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
            if *self.paused.lock().await {
                sleep(Duration::from_millis(200)).await;
                continue;
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

impl LocalClipboardHandler {
    pub fn new() -> Self {
        let factory = create_clipboard().unwrap();
        Self {
            last_content_hash: Arc::new(TokioMutex::new(None)),
            factory,
            paused: Arc::new(TokioMutex::new(false)),
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
            tasks: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn pause(&self) {
        self.cloud.pause().await;
        self.local.pause().await;
    }

    pub async fn resume(&self) {
        self.cloud.resume().await;
        self.local.resume().await;
    }

    pub async fn start(&self) -> Result<()> {
        let cloud_watcher = self.watch_cloud_clipboard().await?;
        let local_watcher = self.watch_local_clipboard().await?;
        let cloud_to_local_handler = self.cloud_to_local_task().await?;
        let local_to_cloud_handler = self.local_to_cloud_task().await?;

        if let Ok(mut tasks) = self.tasks.write() {
            tasks.extend(vec![
                cloud_watcher,
                local_watcher,
                cloud_to_local_handler,
                local_to_cloud_handler,
            ]);
        }

        Ok(())
    }

    async fn watch_cloud_clipboard(&self) -> Result<JoinHandle<()>> {
        let cloud = self.cloud.clone();
        let queue = Arc::clone(&self.cloud_to_local_queue);

        let cloud_clipboard_watcher = task::spawn(async move {
            loop {
                if let Ok(content) = cloud.pull(Some(Duration::from_secs(1))).await {
                    debug!("Watch new content from cloud: {}", content);
                    let mut queue = queue.lock().await;
                    queue.push_back(content);
                }
                sleep(Duration::from_millis(100)).await;
            }
        });

        Ok(cloud_clipboard_watcher)
    }

    async fn watch_local_clipboard(&self) -> Result<JoinHandle<()>> {
        let local = Arc::clone(&self.local);
        let queue = Arc::clone(&self.local_to_cloud_queue);

        let local_clipboard_watcher = task::spawn(async move {
            loop {
                let result = local.pull(Some(Duration::from_secs(1))).await;
                match result {
                    Ok(payload) => {
                        info!("Watch new content from local: {}", payload);
                        let mut queue = queue.lock().await;
                        queue.push_back(payload);
                    }
                    Err(_) => {
                        // TODO: pull 拉取超时，会频繁打印错误日志. 需要自定义错误类型，如果是超时，则不打印错误日志
                        // error!("Failed to pull from local: {}", e);
                        continue;
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
        });

        Ok(local_clipboard_watcher)
    }

    async fn cloud_to_local_task(&self) -> Result<JoinHandle<()>> {
        let local = Arc::clone(&self.local);
        let queue = Arc::clone(&self.cloud_to_local_queue);
        let last_content_hash: Arc<RwLock<Option<String>>> = Arc::clone(&self.last_content_hash);

        let cloud_to_local_handler = task::spawn(async move {
            loop {
                let payload = {
                    let mut queue = queue.lock().await;
                    queue.pop_front()
                };

                if let Some(p) = payload {
                    info!("Push to local: {}", p);
                    let content_hash = p.hash();
                    let mut retry_count = 0;
                    let max_retries = 5;

                    while retry_count < max_retries {
                        match local.write(p.clone()).await {
                            Ok(_) => {
                                info!("Write to local success: {}", p);
                                *last_content_hash.write().unwrap() = Some(content_hash);
                                break;
                            }
                            Err(e) => {
                                retry_count += 1;
                                error!(
                                    "Write to local failed (try {}/{}): {}",
                                    retry_count, max_retries, e
                                );
                                if retry_count < max_retries {
                                    sleep(Duration::from_millis(500)).await;
                                }
                            }
                        }
                    }

                    if retry_count == max_retries {
                        error!("Write to local failed, reached max retries");
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }
        });

        Ok(cloud_to_local_handler)
    }

    async fn local_to_cloud_task(&self) -> Result<JoinHandle<()>> {
        let cloud = self.cloud.clone();
        let queue = Arc::clone(&self.local_to_cloud_queue);
        let content_hash = Arc::clone(&self.last_content_hash);

        let local_to_cloud_handler = task::spawn(async move {
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
                        sleep(Duration::from_millis(100)).await;
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
                sleep(Duration::from_millis(100)).await;
            }
        });

        Ok(local_to_cloud_handler)
    }
}

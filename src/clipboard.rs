use crate::config::CONFIG;
use crate::{message::Payload, network::WebDAVClient};
use arboard::Clipboard as ArboardClipboard;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use log::info;
use rand::Rng;
use std::borrow::Cow;
use std::sync::RwLock;
use std::{
    collections::VecDeque,
    error::Error,
    sync::{Arc, Mutex},
    thread::sleep,
    time::{Duration, Instant},
};
use tokio::task;

#[derive(Clone)]
pub struct CloudClipboardHandler {
    client: Arc<WebDAVClient>,
    last_modified: Arc<RwLock<Option<DateTime<Utc>>>>,
    #[allow(dead_code)]
    share_code: String,
    pub base_path: String,
    // 是否在程序启动后，立即从云端拉取最近的一个内容
    #[allow(dead_code)]
    pull_on_start: bool,
}

#[derive(Clone)]
pub struct LocalClipboardHandler {
    ctx: Arc<Mutex<ArboardClipboard>>,
}

pub struct Clipboard {
    cloud: CloudClipboardHandler,
    local: LocalClipboardHandler,
    cloud_to_local_queue: Arc<Mutex<VecDeque<Payload>>>,
    local_to_cloud_queue: Arc<Mutex<VecDeque<Payload>>>,
}

impl CloudClipboardHandler {
    pub fn new(client: WebDAVClient) -> Self {
        let share_code = Self::generate_share_code();
        let base_path = format!("/uniclipboard/{}", share_code);
        Self {
            client: Arc::new(client),
            share_code,
            base_path,
            last_modified: Arc::new(RwLock::new(None)),
            pull_on_start: true,
        }
    }

    fn generate_share_code() -> String {
        const CHARSET: &[u8] = b"0123456789";
        const CODE_LEN: usize = 6;

        let mut rng = rand::thread_rng();
        (0..CODE_LEN)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
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
    pub async fn pull(&self, timeout: Option<Duration>) -> Result<Payload, Box<dyn Error>> {
        // FIXME: 当前的逻辑，如果是在程序首次启动后，就会从云端拉取最新的
        // 应该给出选项，在程序启动后，是否立即从云端拉取最近的一个内容
        let start_time = Instant::now();
        let mut latest_file_meta;

        loop {
            if let Some(timeout) = timeout {
                if start_time.elapsed() > timeout {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Timeout while waiting for clipboard change",
                    )));
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
                match *last_modified {
                    None => true,
                    Some(last_modified) => modified > last_modified,
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
        Self {
            ctx: Arc::new(Mutex::new(ArboardClipboard::new().unwrap())),
        }
    }

    fn bytes_to_cow(bytes: &Bytes) -> Cow<'static, [u8]> {
        // 如果 Bytes 是空的，返回一个空的借用切片
        if bytes.is_empty() {
            Cow::Borrowed(&[])
        } else {
            // 将 Bytes 转换为 Vec<u8>
            Cow::Owned(bytes.to_vec())
        }
    }

    /// Writes content to the local clipboard.
    ///
    /// # Arguments
    ///
    /// * `content` - A String that will be written to the clipboard.
    ///
    /// # Returns
    ///
    /// Returns a `Result` which is `Ok(())` if the write operation is successful,
    /// or an `Error` if the operation fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if it fails to set the clipboard contents.
    pub fn write(&self, payload: Payload) -> Result<(), Box<dyn Error>> {
        let mut clipboard = self.ctx.lock().unwrap();
        match payload {
            Payload::Text(text) => {
                let content = String::from_utf8(text.content.to_vec())?;
                clipboard.set_text(content)?;
            }
            Payload::Image(image) => {
                let image = arboard::ImageData {
                    width: 0,  // 需要设置正确的宽度
                    height: 0, // 需要设置正确的高度
                    bytes: Self::bytes_to_cow(&image.content),
                };
                clipboard.set_image(image)?;
            }
        }
        Ok(())
    }

    /// Reads content from the local clipboard.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing a `String` with the clipboard content if successful,
    /// or an error if the operation fails.
    ///
    /// # Errors
    ///
    /// 此函数在无法读取剪贴板内容时将返回错误。
    pub fn read(&self) -> Result<Payload, Box<dyn Error>> {
        let mut clipboard = self.ctx.lock().unwrap();
        if let Ok(text) = clipboard.get_text() {
            Ok(Payload::new_text(
                Bytes::from(text),
                CONFIG.read().unwrap().get_device_id(),
                Utc::now(),
            ))
        } else if let Ok(image) = clipboard.get_image() {
            let image_bytes = match image.bytes {
                Cow::Borrowed(slice) => Bytes::copy_from_slice(slice),
                Cow::Owned(vec) => Bytes::from(vec),
            };
            // 处理图片数据
            Ok(Payload::new_image(
                image_bytes.clone(),
                CONFIG.read().unwrap().get_device_id(),
                Utc::now(),
                image.width,
                image.height,
                "png".to_string(),
                image_bytes.len(),
            ))
        } else {
            Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "不支持的剪贴板内容",
            )))
        }
    }

    /// 持续监视本地剪贴板的变化。
    ///
    /// This method runs in an infinite loop, periodically reading the contents
    /// of the local clipboard. When new content is detected, it is added to
    /// the queue for further processing.
    ///
    /// # Errors
    ///
    /// This function will return an error if:
    /// - There's a failure in reading from the local clipboard
    /// - The queue cannot be locked for updating
    pub fn pull(&mut self, timeout: Option<Duration>) -> Result<Payload, Box<dyn Error>> {
        let latest = self.read()?;
        let start_time = Instant::now();

        loop {
            let current = self.read()?;
            if !current.eq(&latest) {
                return Ok(current);
            }

            if let Some(timeout) = timeout {
                if start_time.elapsed() > timeout {
                    return Err(Box::new(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "Timeout while waiting for clipboard change",
                    )));
                }
            }

            sleep(Duration::from_millis(200));
        }
    }
}

impl Clipboard {
    pub fn new(
        cloud_clipboard_handler: CloudClipboardHandler,
        local_clipboard_handler: LocalClipboardHandler,
    ) -> Self {
        Self {
            cloud: cloud_clipboard_handler,
            local: local_clipboard_handler,
            cloud_to_local_queue: Arc::new(Mutex::new(VecDeque::new())),
            local_to_cloud_queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub async fn watch(&self) -> Result<(), Box<dyn Error>> {
        let _cloud_watcher = self.cloud_watch_task();
        let local_watcher = self.local_watch_task();
        let cloud_to_local_handler = self.cloud_to_local_task();
        let local_to_cloud_handler = self.local_to_cloud_task();

        tokio::try_join!(
            // cloud_watcher,
            local_watcher,
            cloud_to_local_handler,
            local_to_cloud_handler
        )?;

        Ok(())
    }

    async fn cloud_watch_task(&self) -> Result<(), Box<dyn Error>> {
        let cloud = self.cloud.clone();
        let queue = Arc::clone(&self.cloud_to_local_queue);

        task::spawn(async move {
            loop {
                if let Ok(content) = cloud.pull(Some(Duration::from_secs(1))).await {
                    let mut queue = queue.lock().unwrap();
                    queue.push_back(content);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await?;

        Ok(())
    }

    async fn local_watch_task(&self) -> Result<(), Box<dyn Error>> {
        let mut local = self.local.clone();
        let queue = Arc::clone(&self.local_to_cloud_queue);

        task::spawn(async move {
            loop {
                if let Ok(payload) = local.pull(Some(Duration::from_secs(1))) {
                    let mut queue = queue.lock().unwrap();
                    queue.push_back(payload);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await?;

        Ok(())
    }

    async fn cloud_to_local_task(&self) -> Result<(), Box<dyn Error>> {
        let local = self.local.clone();
        let queue = Arc::clone(&self.cloud_to_local_queue);

        task::spawn(async move {
            loop {
                let payload = {
                    let mut queue = queue.lock().unwrap();
                    queue.pop_front()
                };

                if let Some(payload) = payload {
                    local.write(payload).unwrap();
                } else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        })
        .await?;

        Ok(())
    }

    async fn local_to_cloud_task(&self) -> Result<(), Box<dyn Error>> {
        let cloud = self.cloud.clone();
        let queue = Arc::clone(&self.local_to_cloud_queue);

        task::spawn(async move {
            loop {
                let payload = {
                    let mut queue = queue.lock().unwrap();
                    queue.pop_front()
                };

                if let Some(p) = payload {
                    cloud.push(p.clone()).await.unwrap();
                    match p {
                        Payload::Text(text) => {
                            info!("Push text to cloud: {} bytes", text.content.len());
                        }
                        Payload::Image(image) => {
                            let size = image.size as f64 / 1024.0 / 1024.0;
                            info!(
                                "Push image to cloud: Size: {:.2} Mb, Width: {} px, Height: {} px",
                                size, image.width, image.height
                            );
                        }
                    }
                } else {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        })
        .await?;

        Ok(())
    }
}

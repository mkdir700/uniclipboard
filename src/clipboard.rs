use crate::config::CONFIG;
use crate::{message::Payload, network::WebDAVClient};
use bytes::Bytes;
use chrono::Utc;
use clipboard::{ClipboardContext, ClipboardProvider};
use rand::Rng;
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
    pub share_code: String,
    pub base_path: String,
}

#[derive(Clone)]
pub struct LocalClipboardHandler {
    ctx: Arc<Mutex<ClipboardContext>>,
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
    /// Returns a Result which is Ok(()) if the upload is successful, or an Error
    /// if the upload fails.
    ///
    /// # Errors
    ///
    /// This function will return an error if the upload to the WebDAV server fails.
    pub async fn push(&self, payload: Payload) -> Result<(), Box<dyn Error>> {
        self.client.upload(self.base_path.clone(), payload).await?;
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
    pub async fn pull(&self) -> Result<Payload, Box<dyn Error>> {
        let mut latest_file_meta = self
            .client
            .fetch_latest_file_meta(self.base_path.clone())
            .await?;
        let mut latest_modified = latest_file_meta.last_modified;
        loop {
            latest_file_meta = self
                .client
                .fetch_latest_file_meta(self.base_path.clone())
                .await?;

            let file_path = latest_file_meta.get_path();
            let modified = latest_file_meta.last_modified;
            let device_id = latest_file_meta.get_device_id();
            // 如果设备 id 相同,则跳过
            if device_id == CONFIG.read().unwrap().get_device_id() {
                latest_modified = modified;
                continue;
            }

            if modified > latest_modified {
                let payload = self.client.download(file_path).await?;
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
            ctx: Arc::new(Mutex::new(ClipboardContext::new().unwrap())),
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
        // 根据 payload 的 content_type 来决定写入的格式
        // 目前暂时只支持 text 格式
        if payload.content_type != "text" {
            return Err(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Unsupported content type",
            )));
        }
        let content = String::from_utf8(payload.content.to_vec())?;
        self.ctx.lock().unwrap().set_contents(content)?;
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
        let content = self.ctx.lock().unwrap().get_contents()?;
        let payload = Payload::new(
            Bytes::from(content),
            "text".to_string(),
            "local".to_string(), // TODO: get device id
            Utc::now(),
        );
        Ok(payload)
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
        let cloud_watcher = self.cloud_watch_task();
        let local_watcher = self.local_watch_task();
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

    async fn cloud_watch_task(&self) -> Result<(), Box<dyn Error>> {
        let cloud = self.cloud.clone();
        let queue = Arc::clone(&self.cloud_to_local_queue);

        task::spawn(async move {
            loop {
                if let Ok(content) = cloud.pull().await {
                    let mut queue = queue.lock().unwrap();
                    queue.push_back(content);
                }
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
                if let Ok(content) = local.pull(None) {
                    let mut queue = queue.lock().unwrap();
                    queue.push_back(content);
                }
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

                if let Some(payload) = payload {
                    cloud.push(payload).await.unwrap();
                }
            }
        })
        .await?;

        Ok(())
    }
}

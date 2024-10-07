use crate::config::CONFIG;
use crate::message::Payload;
use anyhow::Result;
use bytes::Bytes;
use chrono::Utc;
use clipboard_rs::common::RustImage;
use clipboard_rs::{ClipboardHandler, RustImageData};
use image::GenericImageView;
use image::{ImageBuffer, Rgba, RgbaImage};
use log::debug;
use png::Encoder;
use rayon::prelude::*;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

pub struct RsClipboard(Arc<Mutex<dyn ClipboardContextTrait>>, Arc<Notify>);

pub struct RsClipboardChangeHandler(Arc<Notify>);

// 定义一个 trait 来抽象 ClipboardContext 的行为
pub trait ClipboardContextTrait: Send + Sync {
    fn get_text(&self) -> Result<String>;
    fn set_text(&self, text: String) -> Result<()>;
    fn get_image(&self) -> Result<RustImageData>;
    fn set_image(&self, image: RustImageData) -> Result<()>;
}

impl RsClipboard {
    fn clipboard(&self) -> Arc<Mutex<dyn ClipboardContextTrait>> {
        self.0.clone()
    }

    fn read_text(&self) -> Result<String> {
        let clipboard = self.clipboard();
        let guard = clipboard
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock clipboard: {}", e))?;
        guard
            .get_text()
            .map_err(|e| anyhow::anyhow!("Failed to read text: {}", e))
    }

    fn write_text(&self, text: &str) -> Result<()> {
        let clipboard = self.clipboard();
        let guard = clipboard
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock clipboard: {}", e))?;
        guard
            .set_text(text.to_string())
            .map_err(|e| anyhow::anyhow!("Failed to write text: {}", e))
    }

    fn read_image(&self) -> Result<RustImageData> {
        let clipboard = self.clipboard();
        let guard = clipboard
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock clipboard: {}", e))?;
        let image = guard
            .get_image()
            .map_err(|e| anyhow::anyhow!("Failed to read image: {}", e))?;
        Ok(image)
    }

    fn write_image(&self, image: RustImageData) -> Result<()> {
        let clipboard = self.clipboard();
        let guard = clipboard
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock clipboard: {}", e))?;
        guard
            .set_image(image)
            .map_err(|e| anyhow::anyhow!("Failed to set image: {}", e))
    }

    /// 并行转换图片为 png 格式
    fn parallel_convert_image(&self, image: RustImageData) -> Result<Vec<u8>> {
        let img = image
            .to_rgba8()
            .map_err(|e| anyhow::anyhow!("Failed to convert image: {}", e))?;
        let (width, height) = image.get_size();

        // 如果图像很小，不进行并行处理
        if width * height < 1_000_000 {
            return self.convert_image_simple(img, width, height);
        }

        // 并行处理大图像
        let chunk_size = height / rayon::current_num_threads().max(1) as u32 + 1;
        let chunks: Vec<ImageBuffer<Rgba<u8>, Vec<u8>>> = (0..height)
            .step_by(chunk_size as usize)
            .collect::<Vec<u32>>()
            .par_iter()
            .map(|&start_y| {
                let end_y = (start_y + chunk_size).min(height);
                img.view(0, start_y, width, end_y - start_y).to_image()
            })
            .collect();

        // 重新组合图像
        let mut combined = ImageBuffer::new(width, height);
        for (i, chunk) in chunks.into_iter().enumerate() {
            let start_y = i as u32 * chunk_size;
            image::imageops::replace(&mut combined, &chunk, 0, start_y as i64);
        }

        // 编码为PNG
        self.convert_image_simple(combined, width, height)
    }

    fn convert_image_simple(&self, img: RgbaImage, width: u32, height: u32) -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        {
            let mut encoder = Encoder::new(Cursor::new(&mut buffer), width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder.set_compression(png::Compression::Fast);
            let mut writer = encoder
                .write_header()
                .map_err(|e| anyhow::anyhow!("Failed to write PNG header: {}", e))?;
            writer
                .write_image_data(img.into_raw().as_slice())
                .map_err(|e| anyhow::anyhow!("Failed to write PNG data: {}", e))?;
            // writer 在这里被丢弃，结束对 buffer 的借用
        }
        Ok(buffer)
    }

    pub fn read(&self) -> Result<Payload> {
        if let Ok(image) = self.read_image() {
            let (width, height) = image.get_size();
            debug!("开始转换图像");
            let png_bytes = self.parallel_convert_image(image)?;
            debug!("图像转换完成");

            let size = png_bytes.len();
            let device_id = CONFIG.read().unwrap().get_device_id();
            Ok(Payload::new_image(
                Bytes::from(png_bytes),
                device_id,
                Utc::now(),
                width as usize,
                height as usize,
                "png".to_string(),
                size,
            ))
        } else if let Ok(text) = self.read_text() {
            Ok(Payload::new_text(
                Bytes::from(text),
                CONFIG.read().unwrap().get_device_id(),
                Utc::now(),
            ))
        } else {
            Err(anyhow::anyhow!("Clipboard is empty"))
        }
    }

    pub fn write(&self, payload: Payload) -> Result<()> {
        match payload {
            Payload::Image(image) => {
                let image_data = RustImageData::from_bytes(&image.content.to_vec())
                    .map_err(|e| anyhow::anyhow!("Failed to convert image: {}", e))?;
                self.write_image(image_data)
            }
            Payload::Text(text) => {
                let text = String::from_utf8(text.content.to_vec())?;
                self.write_text(&text)
            }
        }
    }

    pub async fn wait_clipboard_change(&self) -> Result<()> {
        self.1.notified().await;
        Ok(())
    }
}

#[cfg(not(test))]
mod production {
    use super::*;
    use clipboard_rs::{Clipboard, ClipboardContext};

    pub struct ClipboardContextWrapper(ClipboardContext);

    impl ClipboardContextTrait for ClipboardContextWrapper {
        fn get_text(&self) -> Result<String> {
            self.0
                .get_text()
                .map_err(|e| anyhow::anyhow!("Failed to get text: {}", e))
        }

        fn set_text(&self, text: String) -> Result<()> {
            self.0
                .set_text(text)
                .map_err(|e| anyhow::anyhow!("Failed to set text: {}", e))
        }

        fn get_image(&self) -> Result<RustImageData> {
            self.0
                .get_image()
                .map_err(|e| anyhow::anyhow!("Failed to get image: {}", e))
        }

        #[cfg(not(target_os = "windows"))]
        fn set_image(&self, image: RustImageData) -> Result<()> {
            self.0
                .set_image(image)
                .map_err(|e| anyhow::anyhow!("Failed to set image: {}", e))
        }

        #[cfg(target_os = "windows")]
        fn set_image(&self, image: RustImageData) -> Result<()> {
            use super::super::utils::PlatformImage;
            use clipboard_win::{formats, set_clipboard};
    
            let platform_image = PlatformImage::new(
                image
                    .get_dynamic_image()
                    .map_err(|e| anyhow::anyhow!("Failed to get dynamic image: {}", e))?,
            );
            let bmp_bytes = platform_image.to_bitmap();
            match set_clipboard(formats::Bitmap, &bmp_bytes) {
                Ok(_) => Ok(()),
                Err(e) => Err(anyhow::anyhow!("Failed to write image: {}", e)),
            }
        }
        
    }

    impl RsClipboard {
        pub fn new(notify: Arc<Notify>) -> Result<Self> {
            Ok(Self(
                Arc::new(Mutex::new(ClipboardContextWrapper(
                    ClipboardContext::new().map_err(|e| {
                        anyhow::anyhow!("Failed to create clipboard context: {}", e)
                    })?,
                ))),
                notify,
            ))
        }
    }
}


impl RsClipboardChangeHandler {
    pub fn new(notify: Arc<Notify>) -> Self {
        Self(notify)
    }
}

impl ClipboardHandler for RsClipboardChangeHandler {
    fn on_clipboard_change(&mut self) {
        debug!("Clipboard changed");
        self.0.notify_waiters();
    }
}

// 在 mock 模块中创建 MockClipboardContext
#[cfg(test)]
mod mock {
    use super::*;
    use image::{DynamicImage, ImageBuffer, Rgba};

    impl RsClipboard {
        pub fn new(notify: Arc<Notify>) -> Result<Self> {
            use self::mock::MockClipboardContext;
            Ok(Self(
                Arc::new(Mutex::new(MockClipboardContext::new())),
                notify,
            ))
        }
    }

    pub struct MockClipboardContext {
        text: Mutex<String>,
        image: Mutex<Option<MockImageData>>,
    }

    #[derive(Clone)]
    struct MockImageData {
        width: u32,
        height: u32,
        data: Vec<u8>,
    }

    impl MockClipboardContext {
        pub fn new() -> Self {
            Self {
                text: Mutex::new(String::new()),
                image: Mutex::new(None),
            }
        }
    }

    impl ClipboardContextTrait for MockClipboardContext {
        fn get_text(&self) -> Result<String> {
            Ok(self.text.lock().unwrap().clone())
        }

        fn set_text(&self, text: String) -> Result<()> {
            *self.text.lock().unwrap() = text;
            Ok(())
        }

        fn get_image(&self) -> Result<RustImageData> {
            let image_data = self.image.lock().unwrap().clone();
            match image_data {
                Some(data) => {
                    let img = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(
                        data.width,
                        data.height,
                        data.data,
                    )
                    .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
                    let dynamic_image = DynamicImage::ImageRgba8(img);
                    Ok(RustImageData::from_dynamic_image(dynamic_image))
                }
                None => Err(anyhow::anyhow!("No image")),
            }
        }

        fn set_image(&self, image: RustImageData) -> Result<()> {
            let dynamic_image = image.get_dynamic_image().unwrap();
            let (width, height) = dynamic_image.dimensions();
            let rgba_image = dynamic_image.to_rgba8();
            let data = rgba_image.into_raw();

            *self.image.lock().unwrap() = Some(MockImageData {
                width,
                height,
                data,
            });
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_read_write_text() {
        let clipboard = RsClipboard::new(Arc::new(Notify::new())).unwrap();
        clipboard.write_text("Hello, world!").unwrap();
        let text = clipboard.read_text().unwrap();
        assert_eq!(text, "Hello, world!");
    }

    #[tokio::test]
    async fn test_read_write_image() {
        let clipboard = RsClipboard::new(Arc::new(Notify::new())).unwrap();
        // 从 test_resources 目录中读取图片
        // 构建测试资源文件的路径
        let mut test_image_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_image_path.push("test_resources");
        test_image_path.push("google.png");
        // 判断文件是否存在
        assert!(test_image_path.exists());
        println!("test_image_path: {}", test_image_path.to_str().unwrap());

        // let img = ImageReader::open(&test_image_path).unwrap().decode().unwrap();
        let image_bytes = fs::read(&test_image_path).unwrap();
        let image_data = RustImageData::from_bytes(&image_bytes).unwrap();
        let size = image_data.get_size();

        clipboard.write_image(image_data).unwrap();

        let read_image = clipboard.read_image().unwrap();
        assert_eq!(read_image.get_size(), size);
    }

    #[tokio::test]
    async fn test_wait_clipboard_change() {
        let notify = Arc::new(Notify::new());
        let clipboard = RsClipboard::new(notify.clone()).unwrap();

        // 创建一个任务来模拟剪贴板变化
        let trigger_change = tokio::spawn(async move {
            // 等待一小段时间，确保主任务已经开始等待
            sleep(Duration::from_millis(100)).await;
            notify.notify_one();
        });

        // 等待剪贴板变化
        let wait_result =
            tokio::time::timeout(Duration::from_secs(1), clipboard.wait_clipboard_change()).await;

        // 确保触发任务完成
        trigger_change.await.unwrap();

        // 检查是否成功等待到剪贴板变化
        assert!(
            wait_result.is_ok(),
            "Timed out waiting for clipboard change"
        );
        assert!(
            wait_result.unwrap().is_ok(),
            "Error while waiting for clipboard change"
        );
    }

    #[tokio::test]
    async fn test_write_read_payload_text() {
        let clipboard = RsClipboard::new(Arc::new(Notify::new())).unwrap();
        let text = "Hello, Payload!";
        let payload = Payload::new_text(
            Bytes::from(text.to_string()),
            "test_device".to_string(),
            Utc::now(),
        );
        clipboard.write(payload).unwrap();

        let read_payload = clipboard.read().unwrap();
        match read_payload {
            Payload::Text(text_payload) => {
                assert_eq!(text_payload.content, Bytes::from(text.to_string()));
            }
            _ => panic!("Expected text payload"),
        }
    }

    #[tokio::test]
    async fn test_write_read_payload_image() {
        let clipboard = RsClipboard::new(Arc::new(Notify::new())).unwrap();
        let mut test_image_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_image_path.push("test_resources");
        test_image_path.push("google.png");

        let image_bytes = fs::read(&test_image_path).unwrap();
        let image_data = RustImageData::from_bytes(&image_bytes).unwrap();
        let (width, height) = image_data.get_size();

        let payload = Payload::new_image(
            Bytes::from(image_bytes.clone()),
            "test_device".to_string(),
            Utc::now(),
            width as usize,
            height as usize,
            "png".to_string(),
            image_bytes.len(),
        );

        clipboard.write(payload).unwrap();

        let read_payload = clipboard.read().unwrap();
        match read_payload {
            Payload::Image(image_payload) => {
                assert_eq!(image_payload.width, width as usize);
                assert_eq!(image_payload.height, height as usize);
                assert_eq!(image_payload.format, "png");
            }
            _ => panic!("Expected image payload"),
        }
    }
}

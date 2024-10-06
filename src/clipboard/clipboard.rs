use anyhow::Result;
use chrono::Utc;
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler, RustImageData};
#[cfg(target_os = "windows")]
use clipboard_win::empty;
#[cfg(target_os = "windows")]
use clipboard_win::{formats, set_clipboard};
use image::GenericImageView;
use image::{ImageBuffer, Rgba, RgbaImage};
use log::debug;
use png::Encoder;
use rayon::prelude::*;
use std::io::Cursor;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{CloseClipboard, OpenClipboard};

#[cfg(target_os = "windows")]
use super::utils::PlatformImage;
use crate::config::CONFIG;
use crate::message::Payload;
use bytes::Bytes;

pub struct RsClipboard(Arc<Mutex<ClipboardContext>>, Arc<Notify>);

pub struct RsClipboardChangeHandler(Arc<Notify>);

impl RsClipboard {
    pub fn new(notify: Arc<Notify>) -> Result<Self> {
        Ok(Self(
            Arc::new(Mutex::new(ClipboardContext::new().map_err(|e| {
                anyhow::anyhow!("Failed to create clipboard context: {}", e)
            })?)),
            notify,
        ))
    }

    fn clipboard(&self) -> Arc<Mutex<ClipboardContext>> {
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

    #[cfg(not(target_os = "windows"))]
    fn write_image(&self, image: RustImageData) -> Result<()> {
        let clipboard = self.clipboard();
        let guard = clipboard
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock clipboard: {}", e))?;
        guard
            .set_image(image)
            .map_err(|e| anyhow::anyhow!("Failed to set image: {}", e))
    }

    #[cfg(target_os = "windows")]
    fn write_image(&self, image: RustImageData) -> Result<()> {
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
}

impl RsClipboard {
    pub async fn wait_clipboard_change(&self) -> Result<()> {
        self.1.notified().await;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
impl RsClipboard {
    fn ensure_clipboard_open() -> Result<()> {
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return Err(anyhow::anyhow!("Failed to open clipboard"));
            }
        }
        Ok(())
    }

    fn ensure_clipboard_closed() -> Result<()> {
        unsafe {
            if CloseClipboard() == 0 {
                return Err(anyhow::anyhow!("Failed to close clipboard"));
            }
        }
        Ok(())
    }

    fn empty_clipboard() -> Result<()> {
        if empty().is_err() {
            return Err(anyhow::anyhow!("Failed to empty clipboard"));
        }
        Ok(())
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

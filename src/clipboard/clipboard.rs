use anyhow::Result;
use chrono::Utc;
use clipboard_rs::common::RustImage;
use clipboard_rs::{Clipboard, ClipboardContext, ClipboardHandler, RustImageData};
#[cfg(target_os = "windows")]
use clipboard_win::empty;
#[cfg(target_os = "windows")]
use clipboard_win::{formats, set_clipboard};
use log::debug;
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;
#[cfg(target_os = "windows")]
use winapi::um::winuser::{CloseClipboard, OpenClipboard};

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
        Self::ensure_clipboard_open()?;
        Self::empty_clipboard()?;
        let bitmap = image
            .to_bitmap()
            .map_err(|e| anyhow::anyhow!("Failed to convert image: {}", e))?;
        let result = set_clipboard(formats::Bitmap, &bitmap.get_bytes().to_vec())
            .map_err(|e| anyhow::anyhow!("Failed to write image: {}", e));
        result
    }
}

impl RsClipboard {
    pub fn read(&self) -> Result<Payload> {
        if let Ok(image) = self.read_image() {
            let png_data = image
                .to_png()
                .map_err(|e| anyhow::anyhow!("Failed to convert image: {}", e))?;
            let png_bytes = png_data.get_bytes().to_vec();
            let size = png_bytes.len();
            let device_id = CONFIG.read().unwrap().get_device_id();
            let (width, height) = image.get_size();
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

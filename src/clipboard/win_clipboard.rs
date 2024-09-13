use super::traits::ClipboardOperations;
use crate::{image::PlatformImage, message::Payload};
use anyhow::Result;
use arboard::Clipboard;
use clipboard_win::{formats, set_clipboard};
use std::sync::{Arc, Mutex};

pub struct WinClipboard(Arc<Mutex<Clipboard>>);

impl WinClipboard {
    pub fn new() -> Result<Self> {
        Ok(Self(Arc::new(Mutex::new(Clipboard::new()?))))
    }
}

impl ClipboardOperations for WinClipboard {
    fn clipboard(&self) -> Arc<Mutex<Clipboard>> {
        self.0.clone()
    }

    fn write_image(&self, image: &PlatformImage) -> Result<()> {
        let image_data = image.to_vec();
        set_clipboard(formats::Bitmap, &image_data)
            .map_err(|e| anyhow::anyhow!("Failed to write image: {}", e))
    }

    fn write(&self, payload: Payload) -> Result<()> {
        match payload {
            Payload::Image(img) => {
                let image_data = PlatformImage::from_bytes(&img.content);
                self.write_image(&image_data)
            }
            Payload::Text(text) => {
                let text_str = String::from_utf8(text.content.to_vec())?;
                self.write_text(&text_str)
            }
        }
    }
}
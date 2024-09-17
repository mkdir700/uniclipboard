use super::traits::ClipboardOperations;
use crate::{image::PlatformImage, message::Payload};
use anyhow::{anyhow, Result};
use arboard::Clipboard;
use clipboard_win::{empty, formats, set_clipboard};
use std::sync::{Arc, Mutex};
use winapi::um::winuser::{CloseClipboard, GetOpenClipboardWindow, OpenClipboard};

pub struct WinClipboard(Arc<Mutex<Clipboard>>);

impl WinClipboard {
    pub fn new() -> Result<Self> {
        Ok(Self(Arc::new(Mutex::new(Clipboard::new()?))))
    }

    #[allow(dead_code)]
    fn ensure_clipboard_open() -> Result<()> {
        unsafe {
            if OpenClipboard(std::ptr::null_mut()) == 0 {
                return Err(anyhow!("Failed to open clipboard"));
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn ensure_clipboard_closed() -> Result<()> {
        unsafe {
            if CloseClipboard() == 0 {
                return Err(anyhow!("Failed to close clipboard"));
            }
        }
        Ok(())
    }

    #[allow(dead_code)]
    fn empty_clipboard() -> Result<()> {
        if empty().is_err() {
            return Err(anyhow!("Failed to empty clipboard"));
        }
        Ok(())
    }
}

impl ClipboardOperations for WinClipboard {
    fn clipboard(&self) -> Arc<Mutex<Clipboard>> {
        self.0.clone()
    }

    fn write_image(&self, image: &PlatformImage) -> Result<()> {
        Self::ensure_clipboard_open()?;
        // https://learn.microsoft.com/en-us/windows/win32/dataxchg/clipboard-operations#clipboard-ownership
        // when it calls the EmptyClipboard function.
        // The window remains the clipboard owner
        // until it is closed or another window empties the clipboard.
        Self::empty_clipboard()?;
        let image_data = image.to_vec();
        let result = set_clipboard(formats::Bitmap, &image_data)
            .map_err(|e| anyhow!("Failed to write image: {}", e));
        result
    }

    fn write(&self, payload: Payload) -> Result<()> {
        match payload {
            Payload::Image(img) => {
                let image_data = PlatformImage::from_bytes(&img.content)?;
                self.write_image(&image_data)
            }
            Payload::Text(text) => {
                let text_str = String::from_utf8(text.content.to_vec())?;
                self.write_text(&text_str)
            }
        }
    }
}

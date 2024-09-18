use super::traits::ClipboardOperations;
use anyhow::Result;
use arboard::Clipboard;
use std::sync::{Arc, Mutex};

pub struct ArboardClipboard(Arc<Mutex<Clipboard>>);

impl ArboardClipboard {
    pub fn new() -> Result<Self> {
        Ok(Self(Arc::new(Mutex::new(Clipboard::new()?))))
    }
}

impl ClipboardOperations for ArboardClipboard {
    fn clipboard(&self) -> Arc<Mutex<Clipboard>> {
        self.0.clone()
    }

    fn read_image(&self) -> Result<DynamicImage> {
        let clipboard = self.clipboard();
        let mut guard = clipboard.lock().unwrap();
        let image = guard
            .get_image()
            .map_err(|e| anyhow::anyhow!("Failed to read image: {}", e))?;
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(
            image.width as u32,
            image.height as u32,
            image.bytes.into(),
        )
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
        Ok(DynamicImage::ImageRgba8(img))
    }

    fn write_image(&self, image: DynamicImage) -> Result<()> {
        let clipboard = self.clipboard();
        let mut guard = clipboard
            .lock()
            .map_err(|e| anyhow::anyhow!("Failed to lock clipboard: {}", e))?;
        guard.set_image(arboard::ImageData {
            width: image.width,
            height: image.height,
            bytes: image.to_vec().into(),
        })?;
        Ok(())
    }
}

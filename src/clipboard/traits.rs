use std::sync::{Arc, Mutex};
use anyhow::Result;
use image::DynamicImage;
use crate::{message::Payload, image::PlatformImage, config::CONFIG};
use arboard::Clipboard;
use bytes::Bytes;
use chrono::Utc;
use image::{ImageBuffer, ImageFormat, Rgba};
use std::io::Cursor;

pub trait ClipboardOperations: Send + Sync {
    fn clipboard(&self) -> Arc<Mutex<Clipboard>>;

    fn read_text(&self) -> Result<String> {
        let clipboard = self.clipboard();
        let mut guard = clipboard.lock().unwrap();
        guard.get_text()
            .map_err(|e| anyhow::anyhow!("Failed to read text: {}", e))
    }

    fn write_text(&self, text: &str) -> Result<()> {
        let clipboard = self.clipboard();
        let mut guard = clipboard.lock().unwrap();
        guard
            .set_text(text)
            .map_err(|e| anyhow::anyhow!("Failed to write text: {}", e))
    }

    fn read_image(&self) -> Result<DynamicImage> {
        let clipboard = self.clipboard();
        let mut guard = clipboard.lock().unwrap();
        let image = guard.get_image().unwrap();
        let raw_image_bytes = image.bytes.to_vec();
        let img = ImageBuffer::<Rgba<u8>, _>::from_raw(
            image.width as u32,
            image.height as u32,
            raw_image_bytes.clone(),
        )
        .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;
        Ok(DynamicImage::ImageRgba8(img))
    }

    fn write_image(&self, image: &PlatformImage) -> Result<()> {
        let clipboard = self.clipboard();
        let mut guard = clipboard.lock().unwrap();
        guard.set_image(arboard::ImageData {
            width: image.width,
            height: image.height,
            bytes: image.to_vec().into(),
        })?;
        Ok(())
    }

    fn read(&self) -> Result<Payload> {
        if let Ok(text) = self.read_text() {
            Ok(Payload::new_text(
                Bytes::from(text),
                CONFIG.read().unwrap().get_device_id(),
                Utc::now(),
            ))
        } else if let Ok(image) = self.read_image() {
            let mut png_data = Vec::new();
            image.write_to(&mut Cursor::new(&mut png_data), ImageFormat::Png)?;
            let size = png_data.len() as usize;
            Ok(Payload::new_image(
                Bytes::from(png_data),
                CONFIG.read().unwrap().get_device_id(),
                Utc::now(),
                image.width() as usize,
                image.height() as usize,
                "png".to_string(),
                size,
            ))
        } else {
            Err(anyhow::anyhow!("Clipboard is empty"))
        }
    }

    fn write(&self, payload: Payload) -> Result<()> {
        match payload {
            Payload::Image(image) => {
                let platform_image = PlatformImage::from_bytes(&image.content);
                self.write_image(&platform_image)
            }
            Payload::Text(text) => {
                let text = String::from_utf8(text.content.to_vec())?;
                self.write_text(&text)
            }
        }
    }
}
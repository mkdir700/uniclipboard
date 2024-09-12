use super::traits::ClipboardOperations;
use crate::message::Payload;
use anyhow::Result;
use arboard::Clipboard;
use crate::image::PlatformImage;

pub struct ArboardClipboard(Clipboard);

impl ArboardClipboard {
    pub fn new() -> Result<Self> {
        Ok(Self(Clipboard::new()?))
    }
}

impl ClipboardOperations for ArboardClipboard {
    fn read_text(&mut self) -> Result<String> {
        // 实现使用 arboard 读取文本的逻辑
        self.0.get_text().map_err(|e| anyhow::anyhow!("Failed to read text: {}", e))
    }

    fn write_text(&mut self, text: &str) -> Result<()> {
        // 实现使用 arboard 写入文本的逻辑
        self.0.set_text(text).map_err(|e| anyhow::anyhow!("Failed to write text: {}", e))
    }

    fn read_image(&mut self) -> Result<Payload> {
        // 实现使用 arboard 读取图片的逻辑
        // 注意：这里需要根据实际情况实现
        unimplemented!("Image reading not implemented yet")
    }

    fn write_image(&mut self, payload: &Payload) -> Result<()> {
        match payload {
            Payload::Image(image) => {
                let platform_image = PlatformImage::from_bytes(&image.content);
                self.0.set_image(arboard::ImageData {
                    width: image.width as usize,
                    height: image.height as usize,
                    bytes: platform_image.to_vec().into(),
                })?;
                Ok(())
            }
            _ => Err(anyhow::anyhow!("Unsupported payload type")),
        }
    }
}
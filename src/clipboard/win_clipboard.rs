#[cfg(windows)]
use super::traits::ClipboardOperations;
use crate::message::Payload;
use anyhow::Result;
use clipboard_win::{formats, set_clipboard};
use crate::image::PlatformImage;

#[cfg(windows)]
pub struct WinClipboard;

#[cfg(windows)]
impl WinClipboard {
    pub fn new() -> Result<Self> {
        Ok(Self)
    }
}

#[cfg(windows)]
impl ClipboardOperations for WinClipboard {
    fn read_text(&self) -> Result<String> {
        // 使用 arboard 读取文本（这里需要一个 ArboardClipboard 实例）
        unimplemented!("Windows 平台上的文本读取仍使用 ArboardClipboard")
    }

    fn write_text(&self, text: &str) -> Result<()> {
        // 使用 arboard 写入文本（这里需要一个 ArboardClipboard 实例）
        unimplemented!("Windows 平台上的文本写入仍使用 ArboardClipboard")
    }

    fn read_image(&self) -> Result<Payload> {
        // 使用 arboard 读取图片（这里需要一个 ArboardClipboard 实例）
        unimplemented!("Windows 平台上的图片读取仍使用 ArboardClipboard")
    }

    fn write_image(&self, image: &Payload) -> Result<()> {
        // 实现使用 clipboard-win 写入图片的逻辑
        match image {
            Payload::Image(img) => {
                // 将 Payload 转换为适合 clipboard-win 的格式
                // 这里需要根据 Payload 的具体实现来调整
                let image_data = PlatformImage::from_image(img);
                set_clipboard(formats::Bitmap, &image_data)?;
                Ok(())
            },
            _ => Err(anyhow::anyhow!("Invalid payload type for image writing")),
        }
    }
}
use super::{ClipboardOperations, ArboardClipboard};
use anyhow::Result;

#[cfg(windows)]
use super::WinClipboard;

pub fn create_clipboard() -> Result<Box<dyn ClipboardOperations>> {
    #[cfg(windows)]
    {
        Ok(Box::new(WinClipboard::new()?))
    }
    #[cfg(not(windows))]
    {
        Ok(Box::new(ArboardClipboard::new()?))
    }
}
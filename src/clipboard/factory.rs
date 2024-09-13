use super::ClipboardOperations;
use anyhow::Result;
use std::sync::Arc;

#[cfg(any(target_os = "macos", target_os = "linux"))]
use super::ArboardClipboard;
#[cfg(windows)]
use super::WinClipboard;

pub fn create_clipboard() -> Result<Arc<dyn ClipboardOperations>> {
    #[cfg(windows)]
    {
        Ok(Arc::new(WinClipboard::new()?))
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        Ok(Arc::new(ArboardClipboard::new()?))
    }
}

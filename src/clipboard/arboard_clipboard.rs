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
}

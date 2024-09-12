pub mod traits;
pub mod arboard_clipboard;
pub mod win_clipboard;
pub mod factory;

pub use traits::ClipboardOperations;
pub use arboard_clipboard::ArboardClipboard;
#[cfg(windows)]
pub use win_clipboard::WinClipboard;
pub use factory::create_clipboard;
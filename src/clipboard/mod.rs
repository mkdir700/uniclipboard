pub mod traits;
pub mod arboard_clipboard;
pub mod win_clipboard;
pub mod factory;

pub use traits::ClipboardOperations;
#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use arboard_clipboard::ArboardClipboard;
#[cfg(windows)]
pub use win_clipboard::WinClipboard;
pub use factory::create_clipboard;
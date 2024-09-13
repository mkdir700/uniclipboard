#[cfg(any(target_os = "macos", target_os = "linux"))]
pub mod arboard_clipboard;
pub mod factory;
pub mod traits;
#[cfg(windows)]
pub mod win_clipboard;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use arboard_clipboard::ArboardClipboard;
pub use factory::create_clipboard;
pub use traits::ClipboardOperations;
#[cfg(windows)]
pub use win_clipboard::WinClipboard;

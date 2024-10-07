mod cli;
mod clipboard;
mod config;
mod device;
mod encrypt;
mod file_metadata;
mod key_mouse_monitor;
mod logger;
mod message;
mod network;
mod remote_sync;
mod uni_clipboard;
mod utils;

use anyhow::Result;
use console::style;
use key_mouse_monitor::KeyMouseMonitor;
use log::{error, info};
use uni_clipboard::UniClipboardBuilder;
use std::sync::Arc;
use std::time::Duration;

use crate::cli::{interactive_input, parse_args};
use crate::clipboard::LocalClipboard;
use crate::config::Config;
use crate::remote_sync::RemoteSyncManagerTrait;
use crate::remote_sync::{RemoteSyncManager, WebSocketSync};

// 新增函数用于显示标志
fn display_banner() {
    let banner = r#"
█ █ █▄ █ █ █▀▀ █   █ █▀█ █▄▄ █▀█ ▄▀█ █▀█ █▀▄
█▄█ █ ▀█ █ █▄▄ █▄▄ █ █▀▀ █▄█ █▄█ █▀█ █▀▄ █▄▀
"#;
    println!("{}", style(banner).cyan().bold());
    println!("{}", style("欢迎使用 UniClipboard！").green());

    // 显示版本号
    let version = env!("CARGO_PKG_VERSION");
    println!(
        "{} {}",
        style("版本:").yellow().bold(),
        style(version).yellow()
    );

    println!();
}

#[tokio::main]
async fn main() -> Result<()> {
    logger::init();

    display_banner();

    let args = parse_args();
    let mut config = Config::load()?;
    if args.interactive {
        interactive_input(&mut config)?;
    }
    config.save()?;

    let key_mouse_monitor = Arc::new(KeyMouseMonitor::new(Duration::from_secs(
        config.key_mouse_monitor_sleep_timeout.unwrap(),
    )));

    let local_clipboard = Arc::new(LocalClipboard::new());
    let remote_sync_manager = Arc::new(RemoteSyncManager::new());
    let websocket_sync = Arc::new(WebSocketSync::new(config.is_server.unwrap()));

    remote_sync_manager.set_sync_handler(websocket_sync).await;

    let app = UniClipboardBuilder::new()
        .set_local_clipboard(local_clipboard)
        .set_remote_sync(remote_sync_manager)
        .set_key_mouse_monitor(key_mouse_monitor)
        .build()?;

    match app.start().await {
        Ok(_) => info!("UniClipboard started successfully"),
        Err(e) => error!("Failed to start UniClipboard: {}", e),
    }
    app.wait_for_stop().await?;
    Ok(())
}

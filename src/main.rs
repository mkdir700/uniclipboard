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
mod web;
mod errors;
use anyhow::Result;
use console::style;
use log::{error, info};
use uni_clipboard::UniClipboardBuilder;
use web::{WebServer, WebsocketMessageHandler};
use std::sync::Arc;

use crate::cli::{interactive_input, parse_args};
use crate::clipboard::LocalClipboard;
use crate::config::Config;
use crate::remote_sync::RemoteSyncManagerTrait;
use crate::remote_sync::{RemoteSyncManager, WebSocketSync};
use crate::web::WebSocketHandler;
use std::net::SocketAddr;

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
    let mut config = Config::load(None)?;
    if args.interactive {
        interactive_input(&mut config)?;
    }
    config.save(None)?;

    // 暂时禁用
    // let key_mouse_monitor = Arc::new(KeyMouseMonitor::new(Duration::from_secs(
    //     config.key_mouse_monitor_sleep_timeout.unwrap(),
    // )));

    let local_clipboard = Arc::new(LocalClipboard::new());
    let remote_sync_manager = Arc::new(RemoteSyncManager::new());
    let websocket_message_handler = Arc::new(WebsocketMessageHandler::new());
    let websocket_handler = Arc::new(WebSocketHandler::new(websocket_message_handler.clone()));
    let websocket_sync = Arc::new(WebSocketSync::new(websocket_message_handler.clone()));
    let webserver = WebServer::new(
        SocketAddr::new(
            config.webserver_addr.unwrap().parse()?,
            config.webserver_port.unwrap(),
        ),
        websocket_handler,
    );

    remote_sync_manager.set_sync_handler(websocket_sync).await;

    let app = UniClipboardBuilder::new()
        .set_webserver(webserver)
        .set_local_clipboard(local_clipboard)
        .set_remote_sync(remote_sync_manager)
        // .set_key_mouse_monitor(key_mouse_monitor)
        .build()?;

    match app.start().await {
        Ok(_) => {
            info!("UniClipboard started successfully");
            app.wait_for_stop().await?;
        },
        Err(e) => error!("Failed to start UniClipboard: {}", e),
    }
    Ok(())
}

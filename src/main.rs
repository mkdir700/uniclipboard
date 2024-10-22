mod cli;
mod clipboard;
mod config;
mod db;
mod device;
mod encrypt;
mod errors;
mod file_metadata;
mod key_mouse_monitor;
mod logger;
mod message;
mod migrations;
mod models;
mod network;
mod remote_sync;
mod schema;
mod uni_clipboard;
mod utils;
mod web;
use anyhow::Result;
use config::get_config_dir;
use console::style;
use db::DB_POOL;
use device::{get_device_manager, Device};
use log::{error, info};
use std::env;
use std::sync::Arc;
use uni_clipboard::UniClipboardBuilder;
use utils::get_local_ip;
use web::{WebServer, WebSocketMessageHandler};

use crate::cli::{interactive_input, parse_args};
use crate::clipboard::LocalClipboard;
use crate::config::Config;
use crate::remote_sync::RemoteSyncManagerTrait;
use crate::remote_sync::{RemoteSyncManager, WebSocketSync};
use crate::web::WebSocketHandler;
use std::net::SocketAddr;

// 新增函数用于显示标志
fn display_banner(local_ip: String) {
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

    println!(
        "{} {}",
        style("本地 IP 地址: ").yellow().bold(),
        style(local_ip).green()
    );

    println!();
}

#[tokio::main]
async fn main() -> Result<()> {
    logger::init();
    let local_ip = get_local_ip();

    display_banner(local_ip.clone());

    let args = parse_args();
    let mut config = Config::load(None)?;
    if args.interactive {
        interactive_input(&mut config)?;
    }
    config.save(None)?;

    let config_dir = get_config_dir()?;
    env::set_var(
        "DATABASE_URL",
        config_dir.join("uniclipboard.db").to_str().unwrap(),
    );

    DB_POOL.run_migrations()?;
    {
        let mut mutex = get_device_manager().lock().unwrap();
        let device = Device::new(
            config.device_id.clone(),
            Some(local_ip.clone()),
            None,
            Some(config.webserver_port.unwrap()),
        );
        mutex.add(device.clone())?;
        mutex.set_self_device(&device)?;
    }

    // 暂时禁用
    // let key_mouse_monitor = Arc::new(KeyMouseMonitor::new(Duration::from_secs(
    //     config.key_mouse_monitor_sleep_timeout.unwrap(),
    // )));

    let local_clipboard = Arc::new(LocalClipboard::new());
    let remote_sync_manager = Arc::new(RemoteSyncManager::new());
    let websocket_message_handler = Arc::new(WebSocketMessageHandler::new());
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
        }
        Err(e) => error!("Failed to start UniClipboard: {}", e),
    }
    Ok(())
}

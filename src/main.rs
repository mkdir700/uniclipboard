mod cli;
mod clipboard;
mod config;
mod connection;
mod context;
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
use crate::context::AppContextBuilder;
use anyhow::Result;
use console::style;
use device::{get_device_manager, Device};
use log::{error, info};
use std::env;
use uni_clipboard::UniClipboardBuilder;
use utils::get_local_ip;

use crate::cli::{interactive_input, parse_args};
use crate::config::Config;

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

    {
        let manager = get_device_manager();
        let device = Device::new(
            config.device_id.clone(),
            Some(local_ip.clone()),
            None,
            Some(config.webserver_port.unwrap()),
        );
        manager.add(device.clone())?;
        manager.set_self_device(&device)?;
    }

    // 创建 AppContext
    let app_context = AppContextBuilder::new(config).build().await?;

    let app = UniClipboardBuilder::new()
        .set_webserver(app_context.webserver)
        .set_local_clipboard(app_context.local_clipboard)
        .set_remote_sync(app_context.remote_sync_manager)
        .set_connection_manager(app_context.connection_manager)
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

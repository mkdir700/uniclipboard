mod cli;
mod clipboard;
mod clipboard_handler;
mod config;
mod device;
mod encrypt;
mod file_metadata;
mod key_mouse_monitor;
mod message;
mod network;
mod remote_sync;
mod uni_clipboard;
mod utils;

use console::style;
use key_mouse_monitor::KeyMouseMonitor;
use log::{error, info};
use std::time::Duration;
use uni_clipboard::UniClipboardBuilder;

use crate::cli::{interactive_input, parse_args};
use crate::clipboard_handler::LocalClipboard;
use crate::config::Config;
use anyhow::Result;
use env_logger::Env;
use network::WebDAVClient;

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
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    display_banner();

    let args = parse_args();
    let mut config = Config::load()?;
    if args.interactive {
        interactive_input(&mut config)?;
    }
    config.save()?;
    // 暂时不使用键鼠监听器
    #[allow(unused_variables)]
    let key_mouse_monitor = KeyMouseMonitor::new(Duration::from_secs(
        config.key_mouse_monitor_sleep_timeout.unwrap(),
    ));

    let app = UniClipboardBuilder::new()
        .set_local_clipboard(LocalClipboard::new())
        .set_websocket_sync(config.is_server.unwrap().clone())
        .build()
        .await?;
    match app.start().await {
        Ok(_) => info!("UniClipboard started successfully"),
        Err(e) => error!("Failed to start UniClipboard: {}", e),
    }
    app.wait_for_stop().await?;
    Ok(())
}

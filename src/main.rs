mod cli;
mod clipboard;
mod clipboard_handler;
mod config;
mod file_metadata;
mod image;
mod message;
mod network;

use crate::cli::{interactive_input, parse_args};
use crate::clipboard_handler::{ClipboardHandler, CloudClipboardHandler, LocalClipboardHandler};
use crate::config::Config;
use crate::network::WebDAVClient;
use anyhow::Result;
use env_logger::Env;
use log::{error, info};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args = parse_args();
    let mut config = Config::load()?;

    let (mut webdav_url, mut username, mut password) = if args.interactive {
        interactive_input()?
    } else {
        (
            args.webdav_url.unwrap_or_default(),
            args.username.unwrap_or_default(),
            args.password.unwrap_or_default(),
        )
    };

    // 如果命令行参数提供的是空值，则从配置文件中获取
    if webdav_url.is_empty() {
        webdav_url = config.get_webdav_url();
    }
    if username.is_empty() {
        username = config.get_username();
    }
    if password.is_empty() {
        password = config.get_password();
    }
    // 验证是否可连接
    let client = WebDAVClient::new(webdav_url.clone(), username.clone(), password.clone()).await?;

    // 验证是否可连接
    let is_connected = client.is_connected().await;
    if is_connected {
        info!(
            "Connected to WebDAV server, device_id: {}",
            config.get_device_id()
        );
    } else {
        error!("Failed to connect to WebDAV server");
        return Err(anyhow::anyhow!("Failed to connect to WebDAV server"));
    }

    config.webdav_url = webdav_url;
    config.username = username;
    config.password = password;
    config.save()?;

    let cloud_clipboard = CloudClipboardHandler::new(client);
    let local_clipboard = LocalClipboardHandler::new();
    let clipboard = ClipboardHandler::new(cloud_clipboard, local_clipboard);
    clipboard.watch().await.unwrap();
    Ok(())
}

mod cli;
mod clipboard;
mod config;
mod message;
mod network;
mod file_metadata;

use crate::cli::parse_args;
use crate::clipboard::{Clipboard, CloudClipboardHandler, LocalClipboardHandler};
use crate::config::Config;
use crate::config::CONFIG;
use crate::network::WebDAVClient;
use anyhow::Result;
use log::info;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let args = parse_args();

    // 尝试加载配置,如果不存在则创建新的
    let config = Config::load().unwrap_or_else(|_| Config {
        device_id: Uuid::new_v4().to_string(),
        webdav_url: args.webdav_url.clone(),
        username: args.username.clone(),
        password: args.password.clone(),
    });

    {
        let mut config = CONFIG.write().unwrap();
        // 如果命令行参数提供了值,则更新配置
        if !args.webdav_url.is_empty() {
            config.webdav_url = args.webdav_url;
        }
        if !args.username.is_empty() {
            config.username = args.username;
        }
        if !args.password.is_empty() {
            config.password = args.password;
        }

        // 保存配置
        config.save()?;
    }

    // 使用更新后的配置创建 WebDAVClient
    let client = WebDAVClient::new(
        config.get_webdav_url(),
        config.get_username(),
        config.get_password(),
    )
    .await?;

    info!(
        "Connected to WebDAV server, device_id: {}",
        config.get_device_id()
    );

    let cloud_clipboard = CloudClipboardHandler::new(client);
    let local_clipboard = LocalClipboardHandler::new();
    let clipboard = Clipboard::new(cloud_clipboard, local_clipboard);
    clipboard.watch().await.unwrap();
    Ok(())
}

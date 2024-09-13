mod cli;
mod clipboard;
mod config;
mod message;
mod network;
mod file_metadata;
mod clipboard_handler;
mod image;

use crate::cli::parse_args;
use crate::clipboard_handler::{ClipboardHandler, CloudClipboardHandler, LocalClipboardHandler};
use crate::config::{CONFIG, Config};
use crate::network::WebDAVClient;
use anyhow::Result;
use log::info;
use env_logger::Env;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let args = parse_args();

    {
        let mut config = Config::load()?;
        // 如果命令行参数提供了值,则更新配置
        if args.webdav_url.is_some() {
            config.webdav_url = args.webdav_url.unwrap();
        }
        if args.username.is_some() {
            config.username = args.username.unwrap();
        }
        if args.password.is_some() {
            config.password = args.password.unwrap();
        }

        // 保存配置
        config.save()?;
    }

    let config = CONFIG.read().unwrap();
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
    let clipboard = ClipboardHandler::new(cloud_clipboard, local_clipboard);
    clipboard.watch().await.unwrap();
    Ok(())
}

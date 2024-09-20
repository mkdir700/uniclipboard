mod cli;
mod clipboard;
mod clipboard_handler;
mod config;
mod device;
mod encrypt;
mod file_metadata;
mod image;
mod key_mouse_monitor;
mod message;
mod network;
mod remote_sync;
mod uni_clipboard;
mod utils;

use log::{error, info};
use std::sync::Arc;

use crate::cli::{interactive_input, parse_args};
use crate::clipboard_handler::LocalClipboard;
use crate::config::Config;
use crate::remote_sync::{RemoteClipboardSync, RemoteSyncManager, WebDavSync, WebSocketSync};
use crate::uni_clipboard::UniClipboard;
use anyhow::Result;
use env_logger::Env;
use network::WebDAVClient;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(Env::default().default_filter_or("debug")).init();
    let args = parse_args();
    let mut config = Config::load()?;
    if args.interactive {
        interactive_input(&mut config)?;
    }
    config.save()?;

    let local_clipboard = LocalClipboard::new();
    let remote_sync: Arc<dyn RemoteClipboardSync> = if config.enable_websocket.unwrap_or(false) {
        let is_server = config.is_server.unwrap();
        Arc::new(WebSocketSync::new(is_server))
    } else if config.enable_webdav.unwrap_or(false) {
        let webdav_client = WebDAVClient::new(
            config.webdav_url.unwrap(),
            config.username.unwrap(),
            config.password.unwrap(),
        )
        .await?;
        Arc::new(WebDavSync::new(webdav_client))
    } else {
        return Err(anyhow::anyhow!("No remote sync enabled"));
    };
    let remote_sync_manager = RemoteSyncManager::new();
    remote_sync_manager.set_sync_handler(remote_sync).await;
    let app = UniClipboard::new(local_clipboard, remote_sync_manager);
    match app.start().await {
        Ok(_) => info!("UniClipboard started successfully"),
        Err(e) => error!("Failed to start UniClipboard: {}", e),
    }
    app.wait_for_stop().await?;
    Ok(())
}

use std::error::Error;
use std::time::Duration;

use log::info;
use tokio::select;
use tokio::signal::ctrl_c;
use tokio::time::sleep;

use crate::clipboard_handler::{ClipboardHandler, CloudClipboardHandler, LocalClipboardHandler};
use crate::config::{Config, CONFIG};
use crate::key_mouse_monitor::KeyMouseMonitor;
use crate::network::WebDAVClient;

pub struct UniClipboard {
    pub key_mouse_monitor: Option<KeyMouseMonitor>,
    pub clipboard_handler: ClipboardHandler,
}

impl UniClipboard {
    pub fn new(webdav_client: WebDAVClient) -> Self {
        let cloud_clipboard_handler = CloudClipboardHandler::new(webdav_client);
        let local_clipboard_handler = LocalClipboardHandler::new();
        let clipboard_handler =
            ClipboardHandler::new(cloud_clipboard_handler, local_clipboard_handler);

        let config = CONFIG.read().unwrap();
        let default_config = Config::default();
        let enable_key_mouse_monitor = config
            .enable_key_mouse_monitor
            .unwrap_or(default_config.enable_key_mouse_monitor.unwrap());

        let key_mouse_monitor: Option<KeyMouseMonitor> = if enable_key_mouse_monitor {
            let key_mouse_monitor_sleep_timeout = config
                .key_mouse_monitor_sleep_timeout
                .unwrap_or(default_config.key_mouse_monitor_sleep_timeout.unwrap());
            Some(KeyMouseMonitor::new(Duration::from_millis(
                key_mouse_monitor_sleep_timeout,
            )))
        } else {
            None
        };

        Self {
            key_mouse_monitor,
            clipboard_handler,
        }
    }

    /// 启动
    ///
    /// 启动键鼠活动监控和剪贴板监控。
    ///
    /// # Returns
    ///
    /// 如果监控成功，则返回 Ok(())。
    ///
    /// # Errors
    ///
    /// 如果键鼠活动监控失败，则返回错误。
    ///
    /// 如果剪贴板监控失败，则返回错误。
    pub async fn start(&self) -> Result<(), Box<dyn Error>> {
        let mut last_is_sleep = false;
        match &self.key_mouse_monitor {
            Some(monitor) => monitor.start().await,
            None => (),
        }

        // 启动剪贴板监控
        self.clipboard_handler.start().await?;

        // 监控系统是否处于休眠状态，并根据状态暂停或恢复剪贴板同步。
        loop {
            select! {
                _ = ctrl_c() => {
                    info!("Received Ctrl+C, stopping...");
                    break;
                }
                _ = async {
                    if let Some(monitor) = &self.key_mouse_monitor {
                        if monitor.is_sleep().await {
                            if !last_is_sleep {
                                self.clipboard_handler.pause().await;
                                last_is_sleep = true;
                                info!("Keyboard and mouse is sleeping, pausing clipboard sync");
                            }
                        } else {
                            if last_is_sleep {
                                self.clipboard_handler.resume().await;
                                last_is_sleep = false;
                                info!("Keyboard and mouse is awake, resuming clipboard sync");
                            }
                        }
                    }
                    sleep(Duration::from_millis(100)).await;
                } => {}
            }
        }

        match &self.key_mouse_monitor {
            Some(monitor) => monitor.stop().await,
            None => (),
        }

        Ok(())
    }
}

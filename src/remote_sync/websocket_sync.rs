use super::traits::RemoteClipboardSync;
use crate::device::{get_device_manager, Device};
use crate::message::WebSocketMessage;
use crate::web::WebSocketHandler;
use crate::config::CONFIG;
use crate::{message::ClipboardSyncMessage, network::WebSocketClient};
use anyhow::Result;
use async_trait::async_trait;
use log::{error, info};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::http::Uri;

#[derive(Clone)]
pub struct WebSocketSync {
    server: Arc<WebSocketHandler>,
    connected_devices: Arc<RwLock<HashMap<String, WebSocketClient>>>,
}

impl WebSocketSync {
    pub fn new(server: Arc<WebSocketHandler>) -> Self {
        Self {
            server,
            connected_devices: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// 监听新设备并建立连接
    async fn listen_new_device(&self) -> Result<()> {
        let server = self.server.clone();
        let self_clone = Arc::new(self.clone());

        tokio::spawn(async move {
            while let Ok(Some(device)) = server.subscribe_new_device().await {
                info!("A new device connected: {}, try to connect...", device);
                if let Err(e) = self_clone.connect_device(&device).await {
                    error!("Failed to connect to device: {}, error: {}", device, e);
                }
                info!("Connected to device: {}", device);
            }
        });
        Ok(())
    }

    /// 连接指定设备
    async fn connect_device(&self, device: &Device) -> Result<()> {
        let uri = format!(
            "ws://{}:{}/ws",
            device.ip.as_ref().unwrap(),
            device.server_port.as_ref().unwrap()
        )
        .parse::<Uri>()
        .unwrap();

        let mut client = WebSocketClient::new(uri);
        client.connect().await?;
        client.register().await?;
        client.sync_device_list().await?;

        let mut connected_devices = self.connected_devices.write().await;
        connected_devices.insert(format!("{}:{}", device.ip.as_ref().unwrap(), device.port.as_ref().unwrap()), client);
        Ok(())
    }

    /// 连接对等设备
    async fn connect_peer_device(&self, peer_device_addr: &str, peer_device_port: u16) -> Result<()> {
        let uri = format!("ws://{}:{}/ws", peer_device_addr, peer_device_port).parse::<Uri>().unwrap();
        let mut client = WebSocketClient::new(uri);
        client.connect().await?;
        client.register().await?;
        let mut connected_devices = self.connected_devices.write().await;
        connected_devices.insert(format!("{}:{}", peer_device_addr, peer_device_port), client);
        Ok(())
    }
}

#[async_trait]
impl RemoteClipboardSync for WebSocketSync {
    /// 暂停远程同步
    ///
    /// 仅客户端会被暂停，服务端不会被暂停
    async fn pause(&self) -> Result<()> {
        self.stop().await
    }
    async fn resume(&self) -> Result<()> {
        self.start().await
    }

    /// 向所有已连接的客户端广播消息
    async fn push(&self, message: ClipboardSyncMessage) -> Result<()> {
        let message = WebSocketMessage::ClipboardSync(message);
        // self.server.broadcast(message, None).await?;
        let mut connected_devices = self.connected_devices.write().await;
        for client in connected_devices.values_mut() {
            client.send_clipboard_sync(&message).await?;
        }
        Ok(())
    }

    /// 从任意已连接的客户端接收剪贴板同步消息
    async fn pull(&self, timeout: Option<Duration>) -> Result<ClipboardSyncMessage> {
        let _ = timeout;
        let clip_message = match self.server.subscribe().await {
            Ok(Some(msg)) => msg,
            Ok(None) => return Err(anyhow::anyhow!("No payload received")),
            Err(e) => return Err(e.into()),
        };
        info!("A new clipboard message received: {}", clip_message);
        Ok(clip_message)
    }

    async fn sync(&self) -> Result<()> {
        // 在这个简单的实现中，sync 可以是一个 no-op
        // 或者可以发送一个特殊的同步消息
        Ok(())
    }

    /// 向已知的设备发起 ws 连接
    ///
    /// 并向其他设备同步当前设备已知的设备列表
    async fn start(&self) -> Result<()> {
        info!("Start to connect to devices");
        // 获取设备管理器的锁
        let device_manager = get_device_manager();
        let devices = device_manager
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock device manager"))?
            .get_all_devices()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        // 如果 devices 为空，则尝试从配置中获取对等设备
        if devices.is_empty() {
            let peer_device_addr;
            let peer_device_port;
            {
                let config = CONFIG.read().unwrap();
                peer_device_addr = config.peer_device_addr.clone();
                peer_device_port = config.peer_device_port;
            }
            if let (Some(peer_device_addr), Some(peer_device_port)) = (peer_device_addr, peer_device_port) {
                info!("Start to connect to peer device: {}:{}", peer_device_addr, peer_device_port);
                match self.connect_peer_device(&peer_device_addr, peer_device_port).await {
                    Ok(_) => info!("Connected to peer device: {}:{}", peer_device_addr, peer_device_port),
                    Err(e) => error!("Failed to connect to peer device: {}:{}, error: {}", peer_device_addr, peer_device_port, e),
                }
            } else {
                info!("No peer device found, skip connecting to peer device");
            }
        } else {
            info!("Start to connect to {} devices", devices.len());
            for device in &devices {
                self.connect_device(device).await?;
            }
            info!("All devices connected");
        }

        // 监听新设备
        self.listen_new_device().await?;
        Ok(())
    }

    /// 断开所有已连接的客户端
    async fn stop(&self) -> Result<()> {
        let mut connected_devices = self.connected_devices.write().await;
        for client in connected_devices.values_mut() {
            client.disconnect().await?;
        }
        Ok(())
    }
}

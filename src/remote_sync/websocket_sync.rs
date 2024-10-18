use super::traits::RemoteClipboardSync;
use crate::config::CONFIG;
use crate::device::{get_device_manager, subscribe_new_devices, Device};
use crate::message::WebSocketMessage;
use crate::web::handlers::websocket_message::WebSocketMessageHandler;
use crate::{message::ClipboardSyncMessage, network::WebSocketClient};
use anyhow::Result;
use async_trait::async_trait;
use log::{error, info};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::http::Uri;

#[derive(Clone)]
pub struct WebSocketSync {
    websocket_message_handler: Arc<WebSocketMessageHandler>,
    peer_device_addr: Option<String>,
    peer_device_port: Option<u16>,
    peer_device_connected: Arc<RwLock<Option<bool>>>,
}

impl WebSocketSync {
    pub fn new(websocket_message_handler: Arc<WebSocketMessageHandler>) -> Self {
        let peer_device_addr;
        let peer_device_port;
        {
            let config = CONFIG.read().unwrap();
            peer_device_addr = config.peer_device_addr.clone();
            peer_device_port = config.peer_device_port.clone();
        }
        let peer_device_connected = {
            if peer_device_addr.is_some() && peer_device_port.is_some() {
                Some(false)
            } else {
                None
            }
        };

        Self {
            websocket_message_handler,
            peer_device_addr: peer_device_addr.clone(),
            peer_device_port: peer_device_port.clone(),
            peer_device_connected: Arc::new(RwLock::new(peer_device_connected)),
        }
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
        client.register(None).await?;
        client.sync_device_list().await?;

        let message_handler = self.websocket_message_handler.clone();
        message_handler
            .add_outgoing_connection(
                format!(
                    "{}:{}",
                    device.ip.as_ref().unwrap(),
                    device.port.as_ref().unwrap()
                ),
                client,
            )
            .await;
        Ok(())
    }

    /// 连接对等设备
    async fn connect_peer_device(&self) -> Result<()> {
        if self.peer_device_addr.is_none() || self.peer_device_port.is_none() {
            return Err(anyhow::anyhow!("Peer device address or port is not set"));
        }

        let peer_device_addr = self.peer_device_addr.clone().unwrap();
        let peer_device_port = self.peer_device_port.clone().unwrap();

        let uri = format!("ws://{}:{}/ws", peer_device_addr, peer_device_port)
            .parse::<Uri>()
            .unwrap();
        let mut client = WebSocketClient::new(uri);
        client.connect().await?;
        *self.peer_device_connected.write().await = Some(true);
        client.register(None).await?;
        self.websocket_message_handler
            .add_outgoing_connection(format!("{}:{}", peer_device_addr, peer_device_port), client)
            .await;
        Ok(())
    }

    /// 监听新增的设备
    async fn listen_new_devices(&self) {
        let mut new_devices_rx = subscribe_new_devices();
        let self_clone = Arc::new(self.clone());
        let peer_device_addr = self_clone.peer_device_addr.clone().unwrap();
        let peer_device_port = self_clone.peer_device_port.clone().unwrap();

        tokio::spawn(async move {
            while let Ok(device) = new_devices_rx.recv().await {
                // 如果 device 是对等设备则不进行连接
                let ip = device.ip.as_ref().unwrap();
                let port = device.port.as_ref().unwrap();
                if ip == peer_device_addr.as_str() && *port == peer_device_port {
                    continue;
                }
                match self_clone.connect_device(&device).await {
                    Ok(_) => info!("A new device connected: {}", device),
                    Err(e) => error!("Failed to connect to new device: {}, error: {}", device, e),
                }
            }
        });
    }
}

// impl WebSocketSync{
/// 监听设备上线
// async fn listen_device_online(&self) -> Result<()> {
//     let websocket_message_handler = self.websocket_message_handler.clone();
//     let self_clone = Arc::new(self.clone());
//     let peer_device_addr = self.peer_device_addr.clone();
//     let peer_device_port = self.peer_device_port.clone();
//     let peer_device_connected = self.peer_device_connected.clone();
//     let message_handler = self.websocket_message_handler.clone();

//     tokio::spawn(async move {
//         while let Ok(Some(device)) = websocket_message_handler.subscribe_device_online().await {
//             // 如果是对等设备连接需要判断连接状态是否为 false
//             let peer_device_disconnnected: bool = {
//                 let connected = peer_device_connected.read().await;
//                 connected.is_none() || !connected.unwrap()
//             };
//             // 与对等设备建立连接时，没有把对等设备加入到 connected_devices 中
//             let peer_device_addr = peer_device_addr.clone().unwrap_or("".to_string());
//             // 如果当前设备的 IP 与对等设备IP 相同且对等设备已经处于连接状态则不做连接操作，否则将导致死循环
//             if peer_device_addr == device.ip.clone().unwrap() && !peer_device_disconnnected {
//                 info!("Peer device already connected: {}, skip...", device);
//                 continue;
//             } else if peer_device_disconnnected {
//                 let peer_device_port = peer_device_port.clone().unwrap();

//                 info!("Peer device disconnected, try reconnect...");

//                 match self_clone.connect_peer_device().await {
//                     Ok(_) => info!(
//                         "Reconnected to peer device: {}:{}",
//                         peer_device_addr, peer_device_port
//                     ),
//                     Err(e) => error!(
//                         "Failed to connect to peer device: {}:{}, error: {}",
//                         peer_device_addr, peer_device_port, e
//                     ),
//                 };
//             } else if message_handler.is_outgoing_connection(&device.id).await {
//                 // 如果该设备 id 是存在与 connected_devices 中，则进行重连
//                 info!("Device {} is exist, try reconnect...", device);
//                 if let Err(e) = self_clone.connect_device(&device).await {
//                     error!("Failed to reconnect to device: {}, error: {}", device, e);
//                 }
//                 info!("Reconnected to device: {}", device);
//             } else {
//                 info!("A device connected: {}, try to connect...", device);
//                 if let Err(e) = self_clone.connect_device(&device).await {
//                     error!("Failed to connect to device: {}, error: {}", device, e);
//                 }
//                 info!("Connected to device: {}", device);
//             }
//         }
//     });
//     Ok(())
// }

// /// 监听设备下线
// async fn listen_device_offline(&self) -> Result<()> {
//     let websocket_message_handler = self.websocket_message_handler.clone();
//     let peer_device_connected = self.peer_device_connected.clone();
//     let peer_device_addr = self.peer_device_addr.clone();
//     let peer_device_port = self.peer_device_port.clone();
//     let message_handler = self.websocket_message_handler.clone();

//     tokio::spawn(async move {
//         while let Ok(Some(addr)) = websocket_message_handler.subscribe_device_offline().await {
//             info!("A device offline: {}", addr);
//             // 这里的 ip 和 port ，是指的是客户端的 ip 和 port
//             // 而 connected_devices 中存储的是对方的 ip 和对方的服务端口
//             // 所以需要查询出设备信息
//             let device = {
//                 let guard = get_device_manager().lock().unwrap();
//                 guard
//                     .get_device_by_ip_and_port(&addr.ip().to_string(), addr.port())
//                     .cloned()
//             };

//             match device {
//                 Some(device) => {
//                     let ip = device.ip.as_ref().unwrap();
//                     let server_port = device.server_port.as_ref().unwrap();

//                     if *ip == peer_device_addr.clone().unwrap_or("".to_string())
//                         && *server_port == peer_device_port.unwrap_or(0)
//                     {
//                         *peer_device_connected.write().await = Some(false);
//                     }
//                     message_handler
//                         .remove_outgoing_connection(&format!("{}:{}", ip, server_port))
//                         .await;
//                 }
//                 None => {
//                     error!("Device not found: {}", addr);
//                 }
//             }
//         }
//     });
//     Ok(())
// }
// }

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
        self.websocket_message_handler
            .broadcast(&message, &None)
            .await?;
        Ok(())
    }

    /// 从任意已连接的客户端接收剪贴板同步消息
    async fn pull(&self, timeout: Option<Duration>) -> Result<ClipboardSyncMessage> {
        let _ = timeout;
        let clip_message = match self
            .websocket_message_handler
            .subscribe_clipboard_sync()
            .await
        {
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
            .get_all_devices_except_self()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        // 如果 devices 为空，则尝试从配置中获取对等设备
        if devices.is_empty() && self.peer_device_connected.read().await.is_some() {
            let peer_device_addr = self.peer_device_addr.clone().unwrap();
            let peer_device_port = self.peer_device_port.clone().unwrap();
            info!(
                "Start to connect to peer device: {}:{}",
                peer_device_addr, peer_device_port
            );
            match self.connect_peer_device().await {
                Ok(_) => info!(
                    "Connected to peer device: {}:{}",
                    peer_device_addr, peer_device_port
                ),
                Err(e) => error!(
                    "Failed to connect to peer device: {}:{}, error: {}",
                    peer_device_addr, peer_device_port, e
                ),
            }
        } else {
            info!("Start to connect to {} devices", devices.len());
            for device in &devices {
                self.connect_device(device).await?;
            }
            info!("All devices connected");
        }

        // 监听设备上下线
        // self.listen_device_online().await?;
        // self.listen_device_offline().await?;
        self.websocket_message_handler
            .start_handle_outgoing_connections_messages()
            .await;

        self.listen_new_devices().await;
        Ok(())
    }

    /// 断开所有已连接的客户端
    async fn stop(&self) -> Result<()> {
        self.websocket_message_handler
            .disconnect_all_outgoing_connections()
            .await;
        Ok(())
    }
}

use crate::config::CONFIG;
use crate::device::get_device_manager;
use crate::device::{subscribe_new_devices, Device, GLOBAL_DEVICE_MANAGER};
use crate::message::{ClipboardSyncMessage, WebSocketMessage};
use crate::network::WebSocketClient;
use anyhow::Result;
use futures::future::join_all;
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, RwLock};
use tokio::task::JoinHandle;
use tokio::time::interval;
use tokio_tungstenite::tungstenite::http::Uri;
use warp::ws::Message;

pub type DeviceId = String;
pub type IpPort = String;
pub type Clients = HashMap<IpPort, mpsc::UnboundedSender<Result<Message, warp::Error>>>;

#[derive(Clone)]
pub struct IncomingConnectionManager {
    connections: Arc<RwLock<Clients>>,
}

impl IncomingConnectionManager {
    fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(Clients::default())),
        }
    }

    pub async fn add_connection(
        &self,
        id: IpPort,
        client: mpsc::UnboundedSender<Result<Message, warp::Error>>,
    ) {
        let mut clients = self.connections.write().await;
        clients.insert(id, client);
    }

    pub async fn remove_connection(&self, id: &IpPort) {
        self.disconnect(id).await;
        let mut clients = self.connections.write().await;
        clients.remove(id);
    }

    pub async fn count(&self) -> usize {
        self.connections.read().await.len()
    }

    async fn disconnect(&self, id: &IpPort) {
        let client = {
            let clients = self.connections.read().await;
            clients.get(&id.clone()).cloned()
        };

        // send offline message
        if let Some(client) = client {
            let message = WebSocketMessage::Offline(id.clone());
            let message_str = serde_json::to_string(&message).unwrap();
            let message = Message::text(message_str);
            let _ = client.send(Ok(message));
            let _ = client.send(Ok(Message::close()));
        }
    }

    /// 断开所有连接
    ///
    /// 向所有已连接的设备发送离线消息
    pub async fn disconnect_all(&self) {
        info!("Disconnecting all connections");
        let _ = self
            .broadcast(&WebSocketMessage::Offline("offline".to_string()), &None)
            .await;

        let clients = {
            let mut clients = self.connections.write().await;
            std::mem::take(&mut *clients)
        };

        let close_connections = clients.into_iter().map(|(id, tx)| async move {
            if let Err(e) = tx.send(Ok(Message::close())) {
                error!("Failed to send close message to client {}: {}", id, e);
            }
            // 等待一小段时间，让客户端有机会处理关闭消息
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        });

        // 并发执行所有关闭操作
        join_all(close_connections).await;

        debug!("All connections have been closed");
    }

    async fn broadcast(
        &self,
        message: &WebSocketMessage,
        excludes: &Option<Vec<String>>,
    ) -> Result<()> {
        let message_str = serde_json::to_string(&message).unwrap();
        let message = Message::text(message_str);

        // 只在短时间内持有锁，仅用于克隆需要的数据
        let futures = {
            let clients_guard = self.connections.read().await;
            clients_guard
                .iter()
                .filter(|(&ref client_id, _)| {
                    if let Some(exclude_ids) = &excludes {
                        !exclude_ids.contains(&client_id.to_string())
                    } else {
                        true
                    }
                })
                .map(|(client_id, tx)| {
                    let message = message.clone();
                    let client_id = client_id.clone();
                    let tx = tx.clone(); // 克隆发送器
                    (client_id, tx, message)
                })
                .collect::<Vec<_>>()
        }; // 锁在这里被释放

        // 在锁释放后执行实际的发送操作
        let send_futures = futures
            .into_iter()
            .map(|(client_id, tx, message)| async move {
                if let Err(e) = tx.send(Ok(message)) {
                    eprintln!("向客户端 {} 发送消息时出错: {:?}", client_id, e);
                    error!("Failed to send message to {}: {}", client_id, e);
                    return Err(e);
                }
                debug!("Sent message to {}", client_id);
                Ok(())
            });

        join_all(send_futures).await;
        Ok(())
    }
}

#[derive(Clone)]
pub struct OutgoingConnectionManager {
    connections: Arc<
        RwLock<
            HashMap<
                DeviceId,
                (
                    Arc<RwLock<WebSocketClient>>,
                    broadcast::Receiver<WebSocketMessage>,
                    tokio::task::JoinHandle<()>,
                    tokio::task::JoinHandle<()>,
                ),
            >,
        >,
    >,
    messages_tx: Arc<broadcast::Sender<(String, WebSocketMessage)>>,
}

impl OutgoingConnectionManager {
    fn new() -> Self {
        let (message_tx, _) = broadcast::channel(20);
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            messages_tx: Arc::new(message_tx),
        }
    }

    /// 连接指定设备
    pub async fn connect_device(&self, device: &Device) -> Result<()> {
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

        info!("Connected to device: {}", device);

        self.add_connection(device.id.clone(), client).await;
        Ok(())
    }

    /// 连接对等设备
    async fn connect_with_peer_device(
        &self,
        peer_device_addr: &str,
        peer_device_port: u16,
    ) -> Result<()> {
        if peer_device_addr.is_empty() || peer_device_port == 0 {
            return Err(anyhow::anyhow!("Peer device address or port is not set"));
        }

        let uri = format!("ws://{}:{}/ws", peer_device_addr, peer_device_port)
            .parse::<Uri>()
            .unwrap();
        let mut client = WebSocketClient::new(uri);
        client.connect().await?;
        client.register(None).await?;
        client.sync_device_list().await?;
        self.add_connection(format!("{}:{}", peer_device_addr, peer_device_port), client)
            .await;
        Ok(())
    }

    pub async fn subscribe_outgoing_connections_message(
        &self,
    ) -> broadcast::Receiver<(String, WebSocketMessage)> {
        self.messages_tx.subscribe()
    }

    /// 添加一个连接
    ///
    /// 创建一个异步任务，将接收到的消息转发到 outgoing_connections_message_tx
    /// 创建一个健康检查任务，定期向所有连接发送 ping 消息，如果某个连接没有响应，则认为该连接已断开
    pub async fn add_connection(&self, id: DeviceId, client: WebSocketClient) {
        let mut clients = self.connections.write().await;
        let message_rx = client.subscribe();
        let arc_client = Arc::new(RwLock::new(client));
        let arc_client_clone = arc_client.clone();
        let outgoing_connections_message_tx = self.messages_tx.clone();
        let id_clone = id.clone();

        // 启动消息转发
        let forward_message_task = tokio::spawn(async move {
            let mut message_rx = { arc_client.clone().read().await.subscribe() };
            loop {
                let message = message_rx.recv().await;
                if let Ok(message) = message {
                    let _ = outgoing_connections_message_tx.send((id.clone(), message));
                }
            }
        });
        // 启动健康检查
        let health_check_task = self.start_health_check().await;

        // 设置设备状态为 online
        if let Err(e) = GLOBAL_DEVICE_MANAGER.set_online(&id_clone) {
            error!("Failed to set device {} online: {}", id_clone, e);
        }

        clients.insert(
            id_clone,
            (
                arc_client_clone,
                message_rx,
                forward_message_task,
                health_check_task,
            ),
        );
    }

    async fn remove_connection(&self, id: &DeviceId) {
        self.disconnect(id).await;
        let mut clients = self.connections.write().await;
        if let Some(client) = clients.get_mut(id) {
            client.2.abort();
            client.3.abort();
            clients.remove(id);
        }
    }

    #[allow(dead_code)]
    pub async fn count(&self) -> usize {
        self.connections.read().await.len()
    }

    async fn disconnect(&self, id: &DeviceId) {
        let ws_client = {
            let clients = self.connections.read().await;
            clients.get(id).map(|conn| conn.0.clone())
        };

        if let Some(ws_client) = ws_client {
            let _ = ws_client.write().await.disconnect().await;
        }
    }

    /// 断开所有连接
    pub async fn disconnect_all(&self) {
        let device_id = CONFIG.read().unwrap().device_id.clone();
        // ?TODO: 是发送 WebSocketMessage::Offline 还是 Message::Close ?
        let _ = self
            .broadcast(&WebSocketMessage::Offline(device_id), &None)
            .await;
    }

    async fn broadcast(
        &self,
        message: &WebSocketMessage,
        excludes: &Option<Vec<String>>,
    ) -> Result<()> {
        let clients = self.connections.read().await;
        let mut errors = Vec::new();

        for (id, client) in clients.iter() {
            if let Some(exclude_ids) = excludes {
                if exclude_ids.contains(id) {
                    continue;
                }
            }

            let client = client.0.read().await;
            match client.send_with_websocket_message(message).await {
                Ok(_) => {}
                Err(e) => {
                    error!("Failed to send message to client {}: {}", id, e);
                    errors.push((id.clone(), e));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to send message to some clients: {:?}",
                errors
            ))
        }
    }

    /// 启动健康检查
    // 如果健康检查失败, 则发送则移除连接
    // 如果健康检查成功, 则不做任何操作
    async fn start_health_check(&self) -> JoinHandle<()> {
        let mut interval = interval(Duration::from_secs(1));
        let self_clone = self.clone();

        tokio::spawn(async move {
            loop {
                interval.tick().await;
                let mut disconnected_devices = Vec::new();
                for (id, client) in self_clone.connections.read().await.iter() {
                    if !client.0.read().await.is_connected() {
                        disconnected_devices.push(id.clone());
                    }
                }

                for device_id in disconnected_devices {
                    self_clone.remove_connection(&device_id).await;
                    info!("Health check: device [{}] is disconnected", device_id);
                }
            }
        })
    }
}

#[derive(Clone)]
pub struct ConnectionManager {
    pub incoming: IncomingConnectionManager,
    pub outgoing: OutgoingConnectionManager,
    device_ip_port_map: Arc<RwLock<HashMap<DeviceId, IpPort>>>,
    // TODO: 需要解耦 clipboard_message_sync_sender
    clipboard_message_sync_sender: Arc<broadcast::Sender<ClipboardSyncMessage>>,
}

impl ConnectionManager {
    pub fn new() -> Self {
        let (clipboard_message_sync_sender, _) = broadcast::channel(100);
        Self {
            incoming: IncomingConnectionManager::new(),
            outgoing: OutgoingConnectionManager::new(),
            device_ip_port_map: Arc::new(RwLock::new(HashMap::new())),
            clipboard_message_sync_sender: Arc::new(clipboard_message_sync_sender),
        }
    }

    /// 监听新增的设备
    ///
    /// 当有新设备上线且未连接时，尝试连接该设备
    async fn listen_new_devices(&self) {
        let mut new_devices_rx = subscribe_new_devices();
        let self_clone = self.clone();

        tokio::spawn(async move {
            while let Ok(device) = new_devices_rx.recv().await {
                let is_connected = self_clone.is_connected(&device).await;
                if is_connected {
                    info!("Device {} is already connected, skip...", device);
                    continue;
                }
                match self_clone.outgoing.connect_device(&device).await {
                    Ok(_) => info!("A new device connected: {}", device),
                    Err(e) => error!("Failed to connect to new device: {}, error: {}", device, e),
                }
            }
        });
    }

    /// 启动
    ///
    /// 1. 尝试连接设备列表中的所有设备
    /// 2. 开启 outgoing 的监听新设备
    pub async fn start(&self) -> Result<()> {
        info!("Start to connect to devices");
        // 获取设备管理器的锁
        let devices = get_device_manager()
            .get_all_devices_except_self()
            .map_err(|_| anyhow::anyhow!("Failed to get all devices"))?;

        let config = CONFIG
            .read()
            .map_err(|_| anyhow::anyhow!("Failed to get config"))?;
        let peer_device_addr = config.peer_device_addr.clone();
        let peer_device_port = config.peer_device_port.clone();

        // 如果 devices 为空，则尝试从配置中获取对等设备
        if devices.is_empty() {
            if let (Some(peer_device_addr), Some(peer_device_port)) =
                (peer_device_addr, peer_device_port)
            {
                info!(
                    "Start to connect to peer device: {}:{}",
                    peer_device_addr, peer_device_port
                );
                match self
                    .outgoing
                    .connect_with_peer_device(&peer_device_addr, peer_device_port)
                    .await
                {
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
                warn!("Peer device address or port is not set, so skip connecting to peer device");
            }
        } else {
            let mut errors = vec![];
            info!("Start to connect to {} devices", devices.len());
            for device in &devices {
                self.outgoing
                    .connect_device(device)
                    .await
                    .unwrap_or_else(|e| {
                        errors.push(e);
                    });
            }
            if !errors.is_empty() {
                warn!("Failed to connect to devices: {:?}", errors);
            } else {
                info!("All devices connected");
            }
        }

        // 开启 outgoing 的监听新设备
        self.listen_new_devices().await;

        info!("Connection manager started");

        Ok(())
    }

    /// 停止
    ///
    /// 1. 断开所有连接
    /// 2. 移除所有连接
    /// 3. 设置所有设备为 offline
    pub async fn stop(&self) {
        self.outgoing.disconnect_all().await;
        self.incoming.disconnect_all().await;

        for (ip_port, _) in self.device_ip_port_map.read().await.iter() {
            self.remove_connection(ip_port).await;
        }

        for (device_id, _) in self.incoming.connections.read().await.iter() {
            self.remove_connection(device_id).await;
        }

        info!("Connection manager stopped");
    }

    pub async fn update_device_ip_port(&self, device_id: DeviceId, ip_port: IpPort) {
        let mut map = self.device_ip_port_map.write().await;
        map.insert(device_id, ip_port);
    }

    pub async fn broadcast(
        &self,
        message: &WebSocketMessage,
        excludes: &Option<Vec<String>>,
    ) -> Result<()> {
        let mut errors: Vec<anyhow::Error> = Vec::new();

        if let Err(e) = self.outgoing.broadcast(message, excludes).await {
            errors.push(e);
        }
        if let Err(e) = self.incoming.broadcast(message, excludes).await {
            errors.push(e);
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "Failed to send message to some clients: {:?}",
                errors
            ))
        }
    }

    // TODO: 需要解耦
    pub async fn send_clipboard_sync(&self, message: ClipboardSyncMessage) {
        let _ = self.clipboard_message_sync_sender.send(message);
    }

    // TODO: 需要解耦
    pub async fn subscribe_clipboard_sync(&self) -> broadcast::Receiver<ClipboardSyncMessage> {
        self.clipboard_message_sync_sender.subscribe()
    }

    pub async fn is_connected(&self, device: &Device) -> bool {
        let device_id = device.id.clone();
        let ip_port = format!(
            "{}:{}",
            device.ip.as_ref().unwrap_or(&"".to_string()),
            device.port.as_ref().unwrap_or(&0)
        );
        self.outgoing
            .connections
            .read()
            .await
            .contains_key(&device_id)
            || self
                .incoming
                .connections
                .read()
                .await
                .contains_key(&ip_port)
    }

    /// The `disconnect` function in Rust asynchronously disconnects a device based on its ID.
    ///
    /// Arguments:
    ///
    /// * `id`: The `id` parameter in the `disconnect` function is of type `DeviceId`, which is a reference
    /// to the identifier of a device.
    // pub async fn disconnect(&self, id: &String) {
    //     let ip_port = self.device_ip_port_map.read().await.get(id).cloned();
    //     if let Some(ip_port) = ip_port {
    //         self.incoming.disconnect(&ip_port).await;
    //     } else {
    //         self.outgoing.disconnect(id).await;
    //     }
    // }

    /// 断开指定设备并移除连接
    ///
    /// `id` 可能是 ip:port 或者 device_id
    pub async fn remove_connection(&self, id: &String) {
        let ip_port = self.device_ip_port_map.read().await.get(id).cloned();
        if let Some(ip_port) = ip_port {
            self.incoming.remove_connection(&ip_port).await;
            let device_id = self.device_ip_port_map.read().await.get(id).cloned();
            if let Some(device_id) = device_id {
                if let Err(e) = GLOBAL_DEVICE_MANAGER.set_offline(&device_id) {
                    error!("Failed to set device {} offline: {}", device_id, e);
                }
            }
        } else {
            self.outgoing.remove_connection(id).await;
            if let Err(e) = GLOBAL_DEVICE_MANAGER.set_offline(id) {
                error!("Failed to set device {} offline: {}", id, e);
            }
        }
    }
}

use crate::device::Device;
use crate::message::{ClipboardSyncMessage, WebSocketMessage};
use crate::network::WebSocketClient;
use anyhow::Result;
use futures::future::join_all;
use log::{debug, error, info};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc, Mutex, RwLock};
use warp::ws::Message;

pub type DeviceId = String;
pub type IpPort = String;
pub type Clients = HashMap<IpPort, mpsc::UnboundedSender<Result<Message, warp::Error>>>;

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

    pub async fn remove_connection(&self, id: IpPort) {
        self.disconnect(&id).await;
        let mut clients = self.connections.write().await;
        clients.remove(&id);
    }

    pub async fn count(&self) -> usize {
        self.connections.read().await.len()
    }

    pub async fn disconnect(&self, id: &IpPort) {
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

    pub async fn disconnect_all(&self) {
        info!("Disconnecting all connections");
        self.broadcast(&WebSocketMessage::Offline("offline".to_string()), &None)
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

pub struct OutgoingConnectionManager {
    connections: Arc<
        RwLock<
            HashMap<
                DeviceId,
                (
                    Arc<RwLock<WebSocketClient>>,
                    broadcast::Receiver<WebSocketMessage>,
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

    pub async fn subscribe_outgoing_connections_message(
        &self,
    ) -> broadcast::Receiver<(String, WebSocketMessage)> {
        self.messages_tx.subscribe()
    }

    /// 添加一个连接
    ///
    /// 并创建一个异步任务，将接收到的消息转发到 outgoing_connections_message_tx
    pub async fn add_connection(&self, id: DeviceId, client: WebSocketClient) {
        let mut clients = self.connections.write().await;
        let message_rx = client.subscribe();
        let arc_client = Arc::new(RwLock::new(client));
        let arc_client_clone = arc_client.clone();
        let outgoing_connections_message_tx = self.messages_tx.clone();
        let id_clone = id.clone();

        let forward_message_task = tokio::spawn(async move {
            let mut message_rx = { arc_client.clone().read().await.subscribe() };
            loop {
                let message = message_rx.recv().await;
                if let Ok(message) = message {
                    let _ = outgoing_connections_message_tx.send((id.clone(), message));
                }
            }
        });
        clients.insert(
            id_clone,
            (arc_client_clone, message_rx, forward_message_task),
        );
    }

    async fn remove_connection(&self, id: &DeviceId) {
        self.disconnect(id).await;
        let mut clients = self.connections.write().await;
        let client = clients.get_mut(id).unwrap();
        client.2.abort();
        clients.remove(id);
    }

    pub async fn count(&self) -> usize {
        self.connections.read().await.len()
    }

    pub async fn disconnect(&self, id: &DeviceId) {
        let ws_client = {
            let clients = self.connections.read().await;
            clients.get(id).map(|conn| conn.0.clone())
        };

        if let Some(ws_client) = ws_client {
            let _ = ws_client.write().await.disconnect().await;
        }
    }

    pub async fn disconnect_all(&self) {
        let _ = self
            .broadcast(&WebSocketMessage::Offline("offline".to_string()), &None)
            .await;
        let mut clients = self.connections.write().await;
        clients.clear();
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
            match client.send_raw(message).await {
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
}

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
    pub async fn disconnect(&self, id: &DeviceId) {
        self.outgoing.disconnect(id).await;
        let ip_port = self.device_ip_port_map.read().await.get(id).cloned();
        if let Some(ip_port) = ip_port {
            self.incoming.disconnect(&ip_port).await;
        }
    }
}

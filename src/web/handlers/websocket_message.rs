use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::config::CONFIG;
use crate::device::get_device_manager;
use crate::device::Device;
use crate::message::{ClipboardSyncMessage, DeviceListData, WebSocketMessage};
use anyhow::Result;
use futures::future::join_all;
use futures::FutureExt;
use log::debug;
use log::{error, info};
use tokio::sync::RwLock;
use tokio::sync::{broadcast, mpsc, Mutex};
use warp::ws::Message;

use super::websocket::Clients;
use crate::network::WebSocketClient;

pub enum MessageSource {
    IpPort(SocketAddr),
    DeviceId(String),
}

#[derive(Clone)]
pub struct WebSocketMessageHandler {
    clipboard_message_sync_sender: Arc<Mutex<mpsc::Sender<ClipboardSyncMessage>>>,
    clipboard_message_sync_receiver: Arc<Mutex<mpsc::Receiver<ClipboardSyncMessage>>>,
    // device_online_sender: Arc<Mutex<mpsc::Sender<Device>>>,
    // device_online_receiver: Arc<Mutex<mpsc::Receiver<Device>>>,
    device_offline_sender: Arc<Mutex<mpsc::Sender<SocketAddr>>>,
    device_offline_receiver: Arc<Mutex<mpsc::Receiver<SocketAddr>>>,
    // 连接到本设备的设备, ip:port -> Client
    incoming_connections: Arc<RwLock<Clients>>,
    // 本设备连接到的设备, device_id -> Client
    outgoing_connections: Arc<
        RwLock<
            HashMap<
                String,
                (
                    Arc<RwLock<WebSocketClient>>,
                    broadcast::Receiver<WebSocketMessage>,
                    tokio::task::JoinHandle<()>,
                ),
            >,
        >,
    >,
    outgoing_connections_message_tx: Arc<Mutex<mpsc::Sender<(String, WebSocketMessage)>>>,
    outgoing_connections_message_rx: Arc<Mutex<mpsc::Receiver<(String, WebSocketMessage)>>>,
}

impl WebSocketMessageHandler {
    pub fn new() -> Self {
        let (clipboard_message_sync_sender, clipboard_message_sync_receiver) = mpsc::channel(100);
        // let (device_online_sender, device_online_receiver) = mpsc::channel(20);
        let (device_offline_sender, device_offline_receiver) = mpsc::channel(20);
        let (outgoing_connections_message_tx, outgoing_connections_message_rx) = mpsc::channel(20);
        Self {
            clipboard_message_sync_sender: Arc::new(Mutex::new(clipboard_message_sync_sender)),
            clipboard_message_sync_receiver: Arc::new(Mutex::new(clipboard_message_sync_receiver)),
            // device_online_sender: Arc::new(Mutex::new(device_online_sender)),
            // device_online_receiver: Arc::new(Mutex::new(device_online_receiver)),
            device_offline_sender: Arc::new(Mutex::new(device_offline_sender)),
            device_offline_receiver: Arc::new(Mutex::new(device_offline_receiver)),
            incoming_connections: Arc::new(RwLock::new(Clients::default())),
            outgoing_connections: Arc::new(RwLock::new(HashMap::new())),
            outgoing_connections_message_tx: Arc::new(Mutex::new(outgoing_connections_message_tx)),
            outgoing_connections_message_rx: Arc::new(Mutex::new(outgoing_connections_message_rx)),
        }
    }

    pub async fn add_incoming_connection(
        &self,
        id: String,
        client: mpsc::UnboundedSender<Result<Message, warp::Error>>,
    ) {
        let mut clients = self.incoming_connections.write().await;
        clients.insert(id, client);
    }

    pub async fn count_incoming_connections(&self) -> usize {
        self.incoming_connections.read().await.len()
    }

    pub async fn remove_incoming_connection(&self, id: String) {
        let mut clients = self.incoming_connections.write().await;
        clients.remove(&id);
    }

    pub async fn disconnect_incoming_connection(&self, id: String, addr: Option<SocketAddr>) {
        self.remove_incoming_connection(id.clone()).await;
        println!("disconnect_incoming_connection: {}", id);
        match addr {
            Some(addr) => {
                let _ = self.device_offline_sender.lock().await.send(addr).await;
            }
            None => {
                error!("Client disconnected, but addr is None");
            }
        }
    }

    async fn broadcast_to_incoming(
        &self,
        message: &WebSocketMessage,
        excludes: &Option<Vec<String>>,
    ) -> Result<()> {
        let message_str = serde_json::to_string(&message).unwrap();
        let message = Message::text(message_str);

        // 只在短时间内持有锁，仅用于克隆需要的数据
        let futures = {
            let clients_guard = self.incoming_connections.read().await;
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
        let send_futures = futures.into_iter().map(|(client_id, tx, message)| {
            async move {
                if let Err(e) = tx.send(Ok(message)) {
                    eprintln!("向客户端 {} 发送消息时出错: {:?}", client_id, e);
                    error!("Failed to send message to {}: {}", client_id, e);
                    return Err(e);
                }
                debug!("Sent message to {}", client_id);
                Ok(())
            }
            .boxed()
        });

        join_all(send_futures).await;
        Ok(())
    }
}

impl WebSocketMessageHandler {
    pub async fn add_outgoing_connection(&self, id: String, client: WebSocketClient) {
        let mut clients = self.outgoing_connections.write().await;
        let message_rx = client.subscribe();
        let arc_client = Arc::new(RwLock::new(client));
        let arc_client_clone = arc_client.clone();
        let outgoing_connections_message_tx = self.outgoing_connections_message_tx.clone();
        let id_clone = id.clone();

        let forward_message_task = tokio::spawn(async move {
            let mut message_rx = { arc_client.clone().read().await.subscribe() };
            loop {
                let message = message_rx.recv().await;
                if let Ok(message) = message {
                    let _ = outgoing_connections_message_tx
                        .lock()
                        .await
                        .send((id.clone(), message))
                        .await;
                }
            }
        });
        clients.insert(
            id_clone,
            (arc_client_clone, message_rx, forward_message_task),
        );
    }

    // pub async fn count_outgoing_connections(&self) -> usize {
    //     self.outgoing_connections.read().await.len()
    // }

    pub async fn remove_outgoing_connection(&self, id: &String) {
        let mut clients = self.outgoing_connections.write().await;
        let client = clients.get_mut(id).unwrap();
        client.2.abort();
        clients.remove(id);
    }

    // pub async fn is_outgoing_connection(&self, id: &String) -> bool {
    //     self.outgoing_connections.read().await.contains_key(id)
    // }

    /// 断开所有 outgoing 连接
    pub async fn disconnect_all_outgoing_connections(&self) {
        let clients = self.outgoing_connections.read().await;
        for client in clients.values() {
            match client.0.write().await.disconnect().await {
                Ok(_) => (),
                Err(e) => error!("Failed to disconnect client: {}", e),
            };
        }
    }

    async fn broadcast_to_outgoing(
        &self,
        message: &WebSocketMessage,
        excludes: &Option<Vec<String>>,
    ) -> Result<()> {
        let clients = self.outgoing_connections.read().await;
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

    /// 开启一个异步任务，从 outgoing_connections 中接收消息并处理
    /// 动态的添加和删除 connections
    pub async fn start_handle_outgoing_connections_messages(&self) {
        let rx = self.outgoing_connections_message_rx.clone();
        let self_clone = Arc::new(self.clone());

        tokio::spawn(async move {
            loop {
                let message = rx.lock().await.recv().await;
                if let Some((device_id, message)) = message {
                    let message = match message.to_json() {
                        Ok(text) => Message::text(text),
                        Err(e) => {
                            error!("Failed to serialize WebSocketMessage: {}, skip handling", e);
                            continue;
                        }
                    };
                    self_clone
                        .handle_message(message, MessageSource::DeviceId(device_id))
                        .await;
                }
            }
        });
    }
}

impl WebSocketMessageHandler {
    /// 向本设备连接到的设备发送消息，以及向连接到本设备的设备发送消息
    pub async fn broadcast(
        &self,
        message: &WebSocketMessage,
        excludes: &Option<Vec<String>>,
    ) -> Result<()> {
        let mut errors: Vec<anyhow::Error> = Vec::new();

        if let Err(e) = self.broadcast_to_outgoing(message, excludes).await {
            errors.push(e);
        }
        if let Err(e) = self.broadcast_to_incoming(message, excludes).await {
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

    pub async fn handle_message(&self, msg: Message, message_source: MessageSource) {
        if msg.is_text() {
            if let Ok(text) = msg.to_str() {
                if text == "connect" {
                    return;
                }
                match serde_json::from_str::<WebSocketMessage>(text) {
                    Ok(websocket_message) => match websocket_message {
                        WebSocketMessage::ClipboardSync(data) => {
                            self.handle_clipboard_sync(data, message_source).await;
                        }
                        WebSocketMessage::DeviceListSync(data) => {
                            self.handle_device_list_sync(data, message_source).await;
                        }
                        WebSocketMessage::Register(register_device_message) => {
                            let device = match message_source {
                                MessageSource::IpPort(addr) => {
                                    Device::new(
                                        register_device_message.id,
                                        Some(addr.ip().to_string()),
                                        Some(addr.port()),
                                        register_device_message.server_port,
                                    )
                                }
                                MessageSource::DeviceId(device_id) => {
                                    // 这种情况应该不会发生，除非是重复注册
                                    error!("Device {} is already registered", device_id);
                                    return;
                                }
                            };
                            self.handle_register(device).await;
                        }
                        WebSocketMessage::Unregister(device_id) => {
                            self.handle_unregister(device_id).await;
                        }
                    },
                    Err(e) => {
                        error!(
                            "Error parsing WebSocket message: {:?}, message: {}",
                            e, text
                        );
                    }
                }
            }
        } else if msg.is_ping() {
            unimplemented!()
            // 返回 pong
            // let sender = self.message_handler.get_incoming_connection(client_id);
            // if let Some(sender) = sender {
            //     let _ = sender.send(Ok(Message::pong(vec![])));
            // }
        }
    }

    pub async fn handle_register(&self, device: Device) {
        let device_manager = get_device_manager();
        {
            let mutex = device_manager.lock();
            if let Ok(mut device_manager) = mutex {
                device_manager.add(device.clone());
            } else {
                error!("Failed to lock device manager");
            }
        }
        // 广播设备列表
        let device_list: Vec<Device> = {
            let device_manager = get_device_manager();
            device_manager
                .lock()
                .unwrap()
                .get_all_devices()
                .into_iter()
                .map(|device| device.clone())
                .collect()
        };

        let device_id = {
            let config = CONFIG.read().unwrap();
            config.device_id.clone()
        };

        let message = WebSocketMessage::DeviceListSync(DeviceListData {
            devices: device_list,
            replay_device_ids: vec![device_id.clone()],
        });
        let _ = self
            .broadcast(&message, &Some(vec![device_id.clone()]))
            .await;
        // let _ = self.device_online_sender.lock().await.send(device).await;
    }

    pub async fn handle_unregister(&self, device_id: String) {
        let device_manager = get_device_manager();
        match device_manager.lock() {
            Ok(mut device_manager) => {
                device_manager.remove(&device_id);
            }
            Err(e) => {
                error!("Failed to lock device manager: {}", e);
            }
        }
    }

    pub async fn handle_clipboard_sync(
        &self,
        data: ClipboardSyncMessage,
        message_source: MessageSource,
    ) {
        {
            let tx = self.clipboard_message_sync_sender.lock().await;
            match tx.send(data.clone()).await {
                Ok(_) => (),
                Err(e) => error!("Failed to send clipboard sync message: {}", e),
            }
        }

        let excludes = match message_source {
            MessageSource::IpPort(addr) => vec![format!("{}:{}", addr.ip(), addr.port())],
            MessageSource::DeviceId(device_id) => vec![device_id],
        };

        // 排除同步消息的来源，否则将导致死循环
        let _ = self
            .broadcast(&WebSocketMessage::ClipboardSync(data), &Some(excludes))
            .await;
        info!("Broadcasted clipboard sync to others");
    }

    /// 处理设备列表同步
    pub async fn handle_device_list_sync(
        &self,
        mut data: DeviceListData,
        message_source: MessageSource,
    ) {
        // 合并设备列表并返回新增的设备
        let _ = {
            let device_manager = get_device_manager();
            device_manager
                .lock()
                .map_err(|_| anyhow::anyhow!("Failed to lock device manager"))
                .unwrap()
                .merge_and_get_new(&data.devices)
        };

        // 追加当前设备 ID到 replay_device_ids
        let device_id = {
            let config = CONFIG
                .read()
                .map_err(|e| anyhow::anyhow!("Failed to read config: {}", e))
                .unwrap();
            config.device_id.clone()
        };

        // 如果该设备列表中的 replay_device_ids 包含当前设备 ID，则不进行广播
        if data.replay_device_ids.contains(&device_id) {
            debug!(
                "Device {} is already in replay_device_ids, skip...",
                device_id
            );
            return;
        }

        data.replay_device_ids.push(device_id.clone());
        let excludes1 = data.replay_device_ids.clone();

        let excludes2 = match message_source {
            MessageSource::IpPort(addr) => vec![format!("{}:{}", addr.ip(), addr.port())],
            MessageSource::DeviceId(device_id) => vec![device_id],
        };

        // 合并 excludes1 和 excludes2
        let excludes = excludes1.into_iter().chain(excludes2.into_iter()).collect();

        info!("Broadcasting device list sync to others");
        // 广播给其他设备，用于他们更新设备列表
        let _ = self
            .broadcast(&WebSocketMessage::DeviceListSync(data), &Some(excludes))
            .await;
    }

    /// 订阅来自其他设备的广播-剪贴板同步
    /// 以及本设备接收到的剪贴板同步
    pub async fn subscribe_clipboard_sync(&self) -> Result<Option<ClipboardSyncMessage>> {
        let rx = self.clipboard_message_sync_receiver.clone();
        loop {
            let data = rx.lock().await.recv().await;
            if let Some(data) = data {
                return Ok(Some(data));
            }
        }
    }

    // pub async fn subscribe_device_online(&self) -> Result<Option<Device>> {
    //     let reader = self.device_online_receiver.clone();
    //     loop {
    //         let data = reader.lock().await.recv().await;
    //         if let Some(data) = data {
    //             return Ok(Some(data));
    //         }
    //     }
    // }

    // pub async fn subscribe_device_offline(&self) -> Result<Option<SocketAddr>> {
    //     let reader = self.device_offline_receiver.clone();
    //     loop {
    //         let data = reader.lock().await.recv().await;
    //         if let Some(data) = data {
    //             return Ok(Some(data));
    //         }
    //     }
    // }
}

#[cfg(test)]
mod tests {
    use serial_test::serial;

    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_register_and_unregister() {
        let handler = WebSocketMessageHandler::new();
        let device = Device::new(
            "device1".to_string(),
            Some("127.0.0.1".to_string()),
            Some(8080),
            Some(8080),
        );
        handler.handle_register(device).await;

        let device_manager = get_device_manager();
        let guard = device_manager.try_lock();
        if let Ok(guard) = guard {
            let devices = guard.get_all_devices_except_self();
            assert_eq!(devices.len(), 1);
        } else {
            assert!(false);
        }
        handler.handle_unregister("device1".to_string()).await;
        let guard = device_manager.try_lock();
        if let Ok(guard) = guard {
            let devices = guard.get_all_devices_except_self();
            assert_eq!(devices.len(), 0);
        } else {
            assert!(false);
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_handle_device_list_sync() {
        let handler = WebSocketMessageHandler::new();
        let device1 = Device::new(
            "device1".to_string(),
            Some("127.0.0.1".to_string()),
            Some(8080),
            Some(8080),
        );
        let device2 = Device::new(
            "device2".to_string(),
            Some("127.0.0.1".to_string()),
            Some(8080),
            Some(8080),
        );
        let device3 = Device::new(
            "device3".to_string(),
            Some("127.0.0.1".to_string()),
            Some(8080),
            Some(8080),
        );

        handler
            .handle_device_list_sync(
                DeviceListData {
                    devices: vec![device1, device2, device3],
                    replay_device_ids: vec![],
                },
                MessageSource::DeviceId("device1".to_string()),
            )
            .await;

        let device_manager = get_device_manager();
        let guard = device_manager.try_lock();
        if let Ok(mut guard) = guard {
            let devices = guard.get_all_devices_except_self();
            let len = devices.len();
            assert_eq!(len, 3);
            guard.clear();
            assert_eq!(guard.get_all_devices_except_self().len(), 0);
        } else {
            assert!(false);
        }
    }
}

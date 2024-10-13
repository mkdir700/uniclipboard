use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use crate::config::CONFIG;
use crate::device::get_device_manager;
use crate::device::Device;
use crate::message::{ClipboardSyncMessage, DeviceListData, WebSocketMessage};
use anyhow::Result;
use futures::future::join_all;
use futures::{FutureExt, StreamExt};
use log::debug;
use log::{error, info};
use tokio::sync::RwLock;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

pub type Sessions =
    Arc<RwLock<HashMap<String, mpsc::UnboundedSender<Result<Message, warp::Error>>>>>;

pub struct WebSocketHandler {
    sessions: Sessions,
    clipboard_message_sync_sender: Arc<Mutex<mpsc::Sender<ClipboardSyncMessage>>>,
    clipboard_message_sync_receiver: Arc<Mutex<mpsc::Receiver<ClipboardSyncMessage>>>,
    new_connect_sender: Arc<Mutex<mpsc::Sender<Device>>>,
    new_connect_receiver: Arc<Mutex<mpsc::Receiver<Device>>>,
}

impl WebSocketHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);
        let (new_device_tx, new_device_rx) = mpsc::channel(100);
        Self {
            sessions: Sessions::default(),
            clipboard_message_sync_sender: Arc::new(Mutex::new(tx)),
            clipboard_message_sync_receiver: Arc::new(Mutex::new(rx)),
            new_connect_sender: Arc::new(Mutex::new(new_device_tx)),
            new_connect_receiver: Arc::new(Mutex::new(new_device_rx)),
        }
    }

    pub async fn client_connected(&self, ws: WebSocket, addr: Option<SocketAddr>) {
        let (server_ws_sender, mut client_ws_rcv) = ws.split();
        let (client_sender, client_rcv) = tokio::sync::mpsc::unbounded_channel();
        let client_rcv = UnboundedReceiverStream::new(client_rcv);

        let client_id = match addr {
            Some(addr) => addr.ip().to_string(),
            None => String::new(),
        };
        if client_id.is_empty() {
            error!("Client id is empty");
            return;
        }

        self.sessions
            .write()
            .await
            .insert(client_id.clone(), client_sender);

        tokio::task::spawn(client_rcv.forward(server_ws_sender).map(|result| {
            if let Err(e) = result {
                error!("Error sending websocket msg to client: {}", e);
            }
        }));

        info!("Client {} connected", client_id);
        while let Some(result) = client_ws_rcv.next().await {
            debug!("Received message from {}", client_id);
            let msg = match result {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Error receiving ws message: {}", e);
                    break;
                }
            };
            self.handle_message(client_id.clone(), msg, addr).await;
        }
        info!("Client {} disconnected", client_id);
        self.client_disconnected(client_id).await;
    }

    async fn client_disconnected(&self, client_id: String) {
        self.sessions.write().await.remove(&client_id);
    }

    async fn handle_message(&self, client_id: String, msg: Message, addr: Option<SocketAddr>) {
        if msg.is_text() {
            if let Ok(text) = msg.to_str() {
                if text == "connect" {
                    return;
                }
                match serde_json::from_str::<WebSocketMessage>(text) {
                    Ok(websocket_message) => match websocket_message {
                        WebSocketMessage::ClipboardSync(data) => {
                            self.handle_clipboard_sync(client_id, data).await;
                        }
                        WebSocketMessage::DeviceListSync(data) => {
                            self.handle_device_list_sync(client_id, data).await;
                        }
                        WebSocketMessage::Register(mut device) => {
                            match addr {
                                Some(addr) => {
                                    device.ip = Some(addr.ip().to_string());
                                    device.port = Some(addr.port());
                                }
                                None => (),
                            }
                            self.handle_register(client_id, device).await;
                        }
                        WebSocketMessage::Unregister(device_id) => {
                            self.handle_unregister(client_id, device_id).await;
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
            // 返回 pong
            let sender = self.sessions.read().await.get(&client_id).cloned();
            if let Some(sender) = sender {
                let _ = sender.send(Ok(Message::pong(vec![])));
            }
        }
    }

    async fn handle_clipboard_sync(&self, client_id: String, data: ClipboardSyncMessage) {
        info!("Received clipboard sync from {}: {}", client_id, data);
        {
            let tx = self.clipboard_message_sync_sender.lock().await;
            match tx.send(data.clone()).await {
                Ok(_) => (),
                Err(e) => error!("Failed to send clipboard sync message: {}", e),
            }
        }
        let _ = self
            .broadcast_to_others(client_id, WebSocketMessage::ClipboardSync(data))
            .await;
        info!("Broadcasted clipboard sync to others");
    }

    /// 处理设备列表同步
    async fn handle_device_list_sync(&self, client_id: String, mut data: DeviceListData) {
        info!("Received device list from {}: {:?}", client_id, data);
        // 合并设备列表并返回新增的设备
        let new_devices = {
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
        data.replay_device_ids.push(device_id.clone());
        let excludes = data.replay_device_ids.clone();

        // 发送新增的设备
        for device in new_devices {
            if device.id == device_id {
                continue;
            }
            let _ = self.new_connect_sender.lock().await.send(device).await;
        }

        // 广播给还没收到这个设备列表的设备
        let _ = self
            .broadcast(WebSocketMessage::DeviceListSync(data), Some(excludes))
            .await;
    }

    async fn handle_register(&self, client_id: String, device: Device) {
        info!("Received register from {}: {}", client_id, device);
        let device_manager = get_device_manager();
        {
            let mutex = device_manager.lock();
            if let Ok(mut device_manager) = mutex {
                device_manager.add(device.clone());
            } else {
                error!("Failed to lock device manager");
            }
        }
        // 如果是 peer 设备，则不新建链接
        let peer_device_addr;
        {
            let config = CONFIG.read().unwrap();
            peer_device_addr = config.peer_device_addr.clone();
        }
        if device.ip == peer_device_addr || device.ip == Some("127.0.0.1".to_string()) {
            return;
        }
        let _ = self.new_connect_sender.lock().await.send(device).await;
    }

    async fn handle_unregister(&self, client_id: String, device_id: String) {
        info!("Received unregister from {}: {}", client_id, device_id);
        let device_manager = get_device_manager();
        device_manager
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock device manager"))
            .unwrap()
            .remove(&device_id);
    }

    async fn broadcast_to_others(
        &self,
        sender_id: String,
        message: WebSocketMessage,
    ) -> Result<()> {
        self.broadcast(message, Some(vec![sender_id])).await
    }

    /// 向所有已连接到本机的设备广播消息
    pub async fn broadcast(
        &self,
        message: WebSocketMessage,
        excludes: Option<Vec<String>>,
    ) -> Result<()> {
        let message_str = serde_json::to_string(&message).unwrap();
        let message = Message::text(message_str);

        // 只在短时间内持有锁，仅用于克隆需要的数据
        let futures = {
            let clients_guard = self.sessions.read().await;
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

    /// 用于外部订阅，接收消息
    pub async fn subscribe(&self) -> Result<Option<ClipboardSyncMessage>> {
        let reader = self.clipboard_message_sync_receiver.clone();
        loop {
            let data = reader.lock().await.recv().await;
            if let Some(data) = data {
                return Ok(Some(data));
            }
        }
    }

    /// 用于外部订阅，接收新增的设备
    pub async fn subscribe_new_device(&self) -> Result<Option<Device>> {
        let reader = self.new_connect_receiver.clone();
        loop {
            let data = reader.lock().await.recv().await;
            if let Some(data) = data {
                return Ok(Some(data));
            }
        }
    }
}

use crate::config::CONFIG;
use crate::device::get_device_manager;
use crate::device::Device;
use crate::message::Payload;
use anyhow::Result;
use futures::stream::SplitSink;
use futures::StreamExt;
use futures_util::SinkExt;
use log::{debug, info};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{accept_async, MaybeTlsStream, WebSocketStream};

#[derive(serde::Deserialize, serde::Serialize)]
pub enum WebSocketMessage {
    Register(Device),
    #[allow(dead_code)]
    Unregister(String),
    #[allow(dead_code)]
    Message(Payload),
}

pub struct WebSocketServer {
    clients: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Message>>>>,
    tx: broadcast::Sender<Payload>,
    reader: Arc<Mutex<mpsc::Receiver<Payload>>>,
    writer: Arc<Mutex<mpsc::Sender<Payload>>>,
}

pub struct WebSocketClient {
    uri: Uri,
    writer: Arc<
        Option<
            Mutex<
                futures_util::stream::SplitSink<
                    WebSocketStream<MaybeTlsStream<TcpStream>>,
                    Message,
                >,
            >,
        >,
    >,

    reader: Arc<
        Option<
            Mutex<futures_util::stream::SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>>,
        >,
    >,
}

impl WebSocketServer {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(100);
        let (writer, reader) = mpsc::channel(100);
        Self {
            clients: Arc::new(Mutex::new(HashMap::new())),
            tx,
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
        }
    }

    pub async fn run(&self, addr: &str) -> Result<()> {
        let listener = TcpListener::bind(addr).await?;
        info!("WebSocket server listening on: {}", addr);

        while let Ok((stream, addr)) = listener.accept().await {
            let ws_stream = accept_async(stream).await?;
            let clients = Arc::clone(&self.clients);
            let tx = self.tx.clone();
            let writer = Arc::clone(&self.writer);
            tokio::spawn(async move {
                Self::handle_connection(ws_stream, addr, clients, tx, writer).await;
            });
        }

        Ok(())
    }

    async fn handle_connection(
        ws_stream: WebSocketStream<TcpStream>,
        addr: SocketAddr,
        clients: Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Message>>>>,
        tx: broadcast::Sender<Payload>,
        writer: Arc<Mutex<mpsc::Sender<Payload>>>,
    ) {
        let (ws_sender, mut ws_receiver) = ws_stream.split();
        let (client_sender, client_receiver) = mpsc::channel(100);

        // 添加新客户端
        {
            let mut clients = clients.lock().await;
            clients.insert(addr, client_sender);
        }

        let mut rx = tx.subscribe();

        // 处理发送消息的任务
        let send_task = tokio::spawn(Self::forward_messages(client_receiver, ws_sender));

        loop {
            tokio::select! {
                msg = ws_receiver.next() => {
                    match msg {
                        Some(Ok(Message::Text(text))) => {
                            // 是否为 连接 信息
                            match serde_json::from_str::<WebSocketMessage>(&text) {
                                Ok(WebSocketMessage::Register(device)) => {
                                    let mut device_manager = get_device_manager().lock().unwrap();
                                    let ip = device.ip.unwrap_or_else(|| addr.ip().to_string());
                                    let port = device.port.unwrap_or_else(|| addr.port());
                                    device_manager.add(Device::new(
                                        device.id,
                                        Some(ip.to_string()),
                                        Some(port),
                                    ));
                                    info!("Device registered: {}", addr);
                                    continue;
                                }
                                _ => {}
                            }

                            if let Ok(payload) = serde_json::from_str::<Payload>(&text) {
                                info!("Received payload: {}", payload);
                                let writer = writer.lock().await;
                                let _ = writer.send(payload).await;
                                // TODO: tx 暂时无用
                                // let _ = tx.send(payload);
                            }
                        }
                        Some(Ok(Message::Close(_))) | None => break,
                        _ => {}
                    }
                }
                Ok(payload) = rx.recv() => {
                    let message = serde_json::to_string(&payload).unwrap();
                    if Self::send_message_to_client(&clients, &addr, Message::Text(message)).await.is_err() {
                        break;
                    }
                }
            }
        }

        // 取消发送任务
        send_task.abort();

        // 移除断开连接的客户端
        clients.lock().await.remove(&addr);
    }

    async fn forward_messages(
        mut receiver: mpsc::Receiver<Message>,
        mut sender: SplitSink<WebSocketStream<TcpStream>, Message>,
    ) {
        while let Some(message) = receiver.recv().await {
            if sender.send(message).await.is_err() {
                break;
            }
        }
    }

    async fn send_message_to_client(
        clients: &Arc<Mutex<HashMap<SocketAddr, mpsc::Sender<Message>>>>,
        addr: &SocketAddr,
        message: Message,
    ) -> Result<(), ()> {
        let sender = {
            let clients = clients.lock().await;
            clients.get(addr).cloned()
        };

        if let Some(sender) = sender {
            sender.send(message).await.map_err(|_| ())
        } else {
            Err(())
        }
    }

    #[allow(dead_code)]
    pub async fn client_count(&self) -> usize {
        self.clients.lock().await.len()
    }

    #[allow(dead_code)]
    pub async fn connected_clients(&self) -> Vec<SocketAddr> {
        self.clients.lock().await.keys().cloned().collect()
    }

    #[allow(dead_code)]
    pub async fn send_to_client(&self, addr: SocketAddr, payload: Payload) -> Result<()> {
        let mut clients = self.clients.lock().await;
        if let Some(ws_stream) = clients.get_mut(&addr) {
            let message = serde_json::to_string(&payload)?;
            ws_stream.send(Message::Text(message)).await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Client not found"))
        }
    }

    pub async fn broadcast(&self, payload: Payload, exclude_addr: Option<String>) -> Result<()> {
        let exclude_addr_clone = exclude_addr.clone();
        debug!(
            "Broadcast payload to all clients: {}, skip: {}",
            payload,
            exclude_addr_clone.unwrap_or("None".to_string())
        );
        let mut clients = self.clients.lock().await;
        info!("Number of clients: {}", clients.len()); // 添加这行来检查客户端数量
        for (addr, ws_stream) in clients.iter_mut() {
            info!("Broadcasting to client: {}", addr);
            if exclude_addr.is_some() && addr.to_string() == *exclude_addr.as_ref().unwrap() {
                continue;
            }
            let message = serde_json::to_string(&payload)?;
            ws_stream.send(Message::Text(message)).await?;
        }
        Ok(())
    }

    /// 用于外部订阅，接收消息
    pub async fn subscribe(&self) -> Result<Option<Payload>> {
        let reader = self.reader.clone();
        loop {
            let payload = reader.lock().await.recv().await;
            if let Some(payload) = payload {
                return Ok(Some(payload));
            }
        }
    }
}

impl WebSocketClient {
    pub fn new(uri: Uri) -> Self {
        WebSocketClient {
            uri,
            writer: Arc::new(None),
            reader: Arc::new(None),
        }
    }

    pub async fn send(&self, payload: Payload) -> Result<()> {
        let message = serde_json::to_string(&payload)?;
        if let Some(writer) = self.writer.as_ref() {
            let mut writer = writer.lock().await;
            writer.send(Message::Text(message)).await?;
        } else {
            return Err(anyhow::anyhow!(
                "WebSocket error: Not connected, please connect first"
            ));
        }
        Ok(())
    }

    pub async fn receive(&self) -> Result<Payload> {
        let mut reader = if let Some(reader) = self.reader.as_ref() {
            reader.lock().await
        } else {
            return Err(anyhow::anyhow!(
                "WebSocket error: Not connected, please connect first"
            ));
        };
        match reader.next().await {
            Some(Ok(Message::Text(text))) => {
                let payload: Payload = serde_json::from_str(&text)?;
                Ok(payload)
            }
            Some(Ok(Message::Close(_))) => Err(anyhow::anyhow!("WebSocket error: Close")),
            Some(Err(e)) => Err(anyhow::anyhow!("WebSocket error: {}", e)),
            None => Err(anyhow::anyhow!("WebSocket error: None")),
            _ => Err(anyhow::anyhow!("WebSocket error: _")),
        }
    }

    pub async fn connect(&mut self) -> Result<()> {
        let (ws_stream, _response) = connect_async(self.uri.clone()).await?;
        let (mut writer, reader) = ws_stream.split();

        // 发送连接消息
        writer.send(Message::Text("connect".to_string())).await?;

        // 更新 writer
        if let Some(arc_writer) = Arc::get_mut(&mut self.writer) {
            *arc_writer = Some(Mutex::new(writer));
        } else {
            self.writer = Arc::new(Some(Mutex::new(writer)));
        }

        // 更新 reader
        if let Some(arc_reader) = Arc::get_mut(&mut self.reader) {
            *arc_reader = Some(Mutex::new(reader));
        } else {
            self.reader = Arc::new(Some(Mutex::new(reader)));
        }

        Ok(())
    }

    pub async fn disconnect(&self) -> Result<()> {
        let mut writer = if let Some(writer) = self.writer.as_ref() {
            writer.lock().await
        } else {
            return Err(anyhow::anyhow!(
                "WebSocket error: Not connected, please connect first"
            ));
        };
        writer.close().await?;
        Ok(())
    }

    pub async fn register(&self) -> Result<()> {
        let device_id = CONFIG.read().unwrap().device_id.clone();
        let web_socket_message = WebSocketMessage::Register(Device::new(device_id, None, None));
        let message = serde_json::to_string(&web_socket_message)?;
        let mut writer = if let Some(writer) = self.writer.as_ref() {
            writer.lock().await
        } else {
            return Err(anyhow::anyhow!(
                "WebSocket error: Not connected, please connect first"
            ));
        };
        writer.send(Message::Text(message)).await?;
        Ok(())
    }
}

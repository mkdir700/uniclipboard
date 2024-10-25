use crate::config::CONFIG;
use crate::device::get_device_manager;
use crate::device::Device;
use crate::message::ClipboardSyncMessage;
use crate::message::DeviceSyncInfo;
use crate::message::DevicesSyncMessage;
use crate::message::RegisterDeviceMessage;
use crate::message::WebSocketMessage;
use anyhow::Result;
use futures::StreamExt;
use futures_util::SinkExt;
use log::error;
use log::info;
use log::warn;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

#[derive(Debug)]
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
    message_tx: Arc<broadcast::Sender<WebSocketMessage>>,
    message_join_handle: Option<tokio::task::JoinHandle<()>>,
    connected: Arc<AtomicBool>,
}

impl WebSocketClient {
    pub fn new(uri: Uri) -> Self {
        let (tx, _) = broadcast::channel(100);
        WebSocketClient {
            uri,
            writer: Arc::new(None),
            reader: Arc::new(None),
            message_tx: Arc::new(tx),
            message_join_handle: None,
            connected: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 开启一个异步任务，持续的从 reader 中接收消息并转发到 message_tx
    async fn start_collect_messages(&mut self) {
        let reader = self.reader.clone();
        let message_tx = self.message_tx.clone();
        let connected = self.connected.clone();

        self.message_join_handle = Some(tokio::spawn(async move {
            loop {
                if let Some(reader) = reader.as_ref() {
                    let mut reader = reader.lock().await;
                    match reader.next().await {
                        Some(Ok(msg)) => {
                            if let Ok(websocket_msg) =
                                serde_json::from_str(&msg.to_text().unwrap_or_default())
                            {
                                let _ = message_tx.send(websocket_msg);
                            }
                        }
                        Some(Err(e)) => {
                            warn!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            warn!("WebSocket connection closed");
                            break;
                        }
                    }
                } else {
                    warn!("WebSocket reader not available");
                    break;
                }
            }

            info!("WebSocket message handler stopped");
            // TODO!: 修改 connected 状态为 false
            connected.store(false, Ordering::Relaxed);
        }));
    }

    pub async fn stop_message_handler(&mut self) {
        if let Some(handle) = self.message_join_handle.take() {
            handle.abort();
            self.message_join_handle = None;
        }
    }

    /// 订阅消息, 返回一个 Receiver 对象
    pub fn subscribe(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.message_tx.subscribe()
    }

    /// 判断是否连接到服务器
    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Relaxed)
    }

    /// 发送剪贴板同步消息
    /// 如果发送失败, 则尝试重新连接, 并重试3次
    #[allow(dead_code)]
    pub async fn send_clipboard_sync(&mut self, message: &WebSocketMessage) -> Result<()> {
        let mut retries = 0;
        while retries < 3 {
            if let Err(e) = self.send_with_websocket_message(message).await {
                retries += 1;
                error!("Failed to send message, retrying... {}", e);
                // 捕获 IO error: Broken pipe(os error 32)
                if e.to_string().to_lowercase().contains("io error") {
                    info!("WebSocket connection broken, reconnecting...");
                    self.reconnect().await?;
                    info!("Reconnected to server");
                    continue;
                } else {
                    return Err(e);
                }
            } else {
                return Ok(());
            }
        }
        Err(anyhow::anyhow!("Failed to send message after 3 retries"))
    }

    /// 接收剪贴板同步消息
    #[allow(dead_code)]
    pub async fn receive_clipboard_sync(&self) -> Result<Option<ClipboardSyncMessage>> {
        let message = self.subscribe().recv().await?;
        if let WebSocketMessage::ClipboardSync(data) = message {
            return Ok(Some(data));
        }
        Ok(None)
    }

    pub async fn send_with_websocket_message(&self, message: &WebSocketMessage) -> Result<()> {
        let message = message.to_tungstenite_message();
        let mut writer = if let Some(writer) = self.writer.as_ref() {
            writer.lock().await
        } else {
            return Err(anyhow::anyhow!(
                "WebSocket error: Not connected, please connect first"
            ));
        };
        writer.send(message).await?;
        Ok(())
    }

    pub async fn send_raw(&self, message: Message) -> Result<()> {
        let mut writer = if let Some(writer) = self.writer.as_ref() {
            writer.lock().await
        } else {
            return Err(anyhow::anyhow!(
                "WebSocket error: Not connected, please connect first"
            ));
        };
        writer.send(message).await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn receive_raw(&self) -> Result<WebSocketMessage> {
        let message = self.subscribe().recv().await?;
        Ok(message)
    }

    /// 重新连接到服务器
    #[allow(dead_code)]
    pub async fn reconnect(&mut self) -> Result<()> {
        self.disconnect().await?;
        self.connect().await
    }

    /// 连接到服务器
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

        self.start_collect_messages().await;
        self.connected.store(true, Ordering::Relaxed);
        Ok(())
    }

    /// 断开与服务器的连接
    pub async fn disconnect(&mut self) -> Result<()> {
        if let Some(writer) = self.writer.as_ref() {
            let mut writer_guard = writer.lock().await;
            match writer_guard.close().await {
                Ok(_) => (),
                Err(e) => warn!("Failed to close WebSocket writer: {}", e),
            }
            // 在这里显式地释放 writer_guard
            drop(writer_guard);
        } else {
            return Err(anyhow::anyhow!(
                "WebSocket error: Not connected, please connect first"
            ));
        }

        // 现在可以安全地修改 self.writer 和 self.reader
        self.writer = Arc::new(None);
        self.reader = Arc::new(None);
        self.stop_message_handler().await;
        self.connected.store(false, Ordering::Relaxed);
        Ok(())
    }

    /// 向服务器注册当前设备
    /// 如果 device 参数为 None，则默认读取配置文件中的设备信息用于注册
    pub async fn register(&self, device: Option<Device>) -> Result<()> {
        let device = device.unwrap_or_else(|| {
            let (device_id, server_port) = {
                let c = CONFIG.read().unwrap();
                (c.device_id.clone(), c.webserver_port)
            };
            Device::new(device_id, None, None, server_port)
        });

        let web_socket_message = WebSocketMessage::Register(RegisterDeviceMessage::new(
            device.id.clone(),
            device.ip,
            device.server_port,
        ));
        let message = serde_json::to_string(&web_socket_message)?;
        let mut writer = if let Some(writer) = self.writer.as_ref() {
            writer.lock().await
        } else {
            return Err(anyhow::anyhow!(
                "WebSocket error: Not connected, please connect first"
            ));
        };
        writer.send(Message::Text(message)).await?;
        info!("Register [{}] to server({})", device.id, self.uri);
        Ok(())
    }

    /// 向其他设备同步当前设备已知的设备列表
    pub async fn sync_device_list(&self) -> Result<()> {
        let device_id = CONFIG.read().unwrap().device_id.clone();
        let devices = get_device_manager()
            .get_all_devices()
            .map_err(|_| anyhow::anyhow!("Failed to get all devices"))?;

        let web_socket_message = WebSocketMessage::DeviceListSync(DevicesSyncMessage {
            devices: devices.iter().map(|d| DeviceSyncInfo::from(d)).collect(),
            replay_device_ids: vec![device_id],
        });
        self.send_with_websocket_message(&web_socket_message)
            .await?;
        Ok(())
    }

    async fn ping(&self) -> Result<()> {
        self.send_raw(Message::Ping(vec![])).await?;
        Ok(())
    }
}

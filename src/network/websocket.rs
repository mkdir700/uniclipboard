use crate::config::CONFIG;
use crate::device::get_device_manager;
use crate::device::Device;
use crate::message::ClipboardSyncMessage;
use crate::message::DeviceListData;
use crate::message::WebSocketMessage;
use anyhow::Result;
use futures::StreamExt;
use futures_util::SinkExt;
use log::error;
use log::info;
use log::warn;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio::sync::Mutex;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::http::Uri;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

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
        }
    }

    pub async fn start_message_handler(&mut self) {
        let reader = self.reader.clone();
        let message_tx = self.message_tx.clone();

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
                            // TODO: 如果意外断开连接该如何处理？
                            // 1. 尝试重连
                            // 2. 发送消息通知上层
                            error!("WebSocket error: {}", e);
                            break;
                        }
                        None => {
                            error!("WebSocket connection closed");
                            break;
                        }
                    }
                } else {
                    error!("WebSocket reader not available");
                    break;
                }
            }
        }));
    }

    pub fn subscribe(&self) -> broadcast::Receiver<WebSocketMessage> {
        self.message_tx.subscribe()
    }

    /// 发送剪贴板同步消息
    /// 如果发送失败, 则尝试重新连接, 并重试3次
    pub async fn send_clipboard_sync(&mut self, message: &WebSocketMessage) -> Result<()> {
        let mut retries = 0;
        while retries < 3 {
            if let Err(e) = self.send_raw(message).await {
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
    pub async fn receive_clipboard_sync(&self) -> Result<Option<ClipboardSyncMessage>> {
        let message = self.subscribe().recv().await?;
        if let WebSocketMessage::ClipboardSync(data) = message {
            return Ok(Some(data));
        }
        Ok(None)
    }

    pub async fn send_raw(&self, message: &WebSocketMessage) -> Result<()> {
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

    pub async fn receive_raw(&self) -> Result<WebSocketMessage> {
        let message = self.subscribe().recv().await?;
        Ok(message)
    }

    /// 重新连接到服务器
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

        self.start_message_handler().await;

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
        if let Some(handle) = self.message_join_handle.take() {
            handle.abort();
        }
        self.message_join_handle = None;
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

        let web_socket_message = WebSocketMessage::Register(device);
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

    /// 向其他设备同步当前设备已知的设备列表
    pub async fn sync_device_list(&self) -> Result<()> {
        let device_id = CONFIG.read().unwrap().device_id.clone();
        let device_manager = get_device_manager();
        let devices = device_manager
            .lock()
            .map_err(|_| anyhow::anyhow!("Failed to lock device manager"))?
            .get_all_devices_except_self()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        let web_socket_message = WebSocketMessage::DeviceListSync(DeviceListData {
            devices,
            replay_device_ids: vec![device_id],
        });
        self.send_raw(&web_socket_message).await?;
        Ok(())
    }

    // pub async fn start_ping_task(&self, interval_secs: u64) {
    //     let mut ping_interval = interval(Duration::from_secs(interval_secs));
    //     let writer = self.writer.clone();

    //     tokio::spawn(async move {
    //         loop {
    //             tokio::select! {
    //                 _ = ping_interval.tick() => {
    //                     if let Some(writer) = writer.as_ref() {
    //                         let mut writer = writer.lock().await;
    //                         if let Err(e) = writer.send(Message::Ping(vec![])).await {
    //                             error!("发送 ping 失败: {}", e);
    //                             break;
    //                         }
    //                     } else {
    //                         error!("WebSocket 写入器不可用");
    //                         break;
    //                     }
    //                 }
    //             }
    //         }
    //     });
    // }
}

use crate::config::CONFIG;
use crate::device::get_device_manager;
use crate::device::Device;
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
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};
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
}

impl WebSocketClient {
    pub fn new(uri: Uri) -> Self {
        WebSocketClient {
            uri,
            writer: Arc::new(None),
            reader: Arc::new(None),
        }
    }

    /// 发送剪贴板同步消息
    /// 如果发送失败, 则尝试重新连接, 并重试3次
    pub async fn send_clipboard_sync(&mut self, message: &WebSocketMessage) -> Result<()> {
        let mut retries = 0;
        while retries < 3 {
            let message_str = serde_json::to_string(message)?;
            if let Err(e) = self.send_raw(Message::Text(message_str)).await {
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

    pub async fn receive_raw(&self) -> Result<Message> {
        let mut reader = if let Some(reader) = self.reader.as_ref() {
            reader.lock().await
        } else {
            return Err(anyhow::anyhow!(
                "WebSocket error: Not connected, please connect first"
            ));
        };

        match reader.next().await {
            Some(Ok(msg)) => Ok(msg),
            Some(Err(e)) => Err(anyhow::anyhow!("WebSocket error: {}", e)),
            None => Err(anyhow::anyhow!("WebSocket error: Connection closed")),
        }
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
        Ok(())
    }

    /// 向服务器注册当前设备
    pub async fn register(&self) -> Result<()> {
        let (device_id, server_port) = {
            let c = CONFIG.read().unwrap();
            (c.device_id.clone(), c.webserver_port)
        };

        let web_socket_message =
            WebSocketMessage::Register(Device::new(device_id, None, None, server_port));
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
            .get_all_devices()
            .into_iter()
            .cloned()
            .collect::<Vec<_>>();

        let web_socket_message = WebSocketMessage::DeviceListSync(DeviceListData {
            devices,
            replay_device_ids: vec![device_id],
        });
        let text = serde_json::to_string(&web_socket_message)?;
        self.send_raw(Message::Text(text)).await?;
        Ok(())
    }

    pub async fn start_ping_task(&self, interval_secs: u64) {
        let mut ping_interval = interval(Duration::from_secs(interval_secs));
        let writer = self.writer.clone();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = ping_interval.tick() => {
                        if let Some(writer) = writer.as_ref() {
                            let mut writer = writer.lock().await;
                            if let Err(e) = writer.send(Message::Ping(vec![])).await {
                                error!("发送 ping 失败: {}", e);
                                break;
                            }
                        } else {
                            error!("WebSocket 写入器不可用");
                            break;
                        }
                    }
                }
            }
        });
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use bytes::Bytes;
//     use chrono::Utc;
//     use serial_test::serial;
//     use std::time::Duration;

//     // 辅助函数：创建一个WebSocketServer并运行它
//     async fn setup_server() -> WebSocketServer {
//         let server = WebSocketServer::new();
//         let server_clone = server.clone();
//         tokio::spawn(async move {
//             server_clone.run("127.0.0.1:8080").await.unwrap();
//         });
//         tokio::time::sleep(Duration::from_millis(100)).await; // 给服务器一些启动时间
//         server
//     }

//     #[tokio::test]
//     #[serial]
//     async fn test_websocket_server_creation() {
//         let server = WebSocketServer::new();
//         assert!(server.clients.lock().await.is_empty());
//     }

//     #[tokio::test]
//     #[serial]
//     async fn test_websocket_client_creation() {
//         let uri = "ws://127.0.0.1:8080".parse().unwrap();
//         let client = WebSocketClient::new(uri);
//         assert!(client.writer.is_none());
//         assert!(client.reader.is_none());
//     }

//     #[tokio::test]
//     #[serial]
//     async fn test_websocket_connection() {
//         let _server = setup_server().await;

//         let uri = "ws://127.0.0.1:8080".parse().unwrap();
//         let mut client = WebSocketClient::new(uri);

//         assert!(client.connect().await.is_ok());
//         assert!(client.writer.is_some());
//         assert!(client.reader.is_some());
//     }

//     #[tokio::test]
//     #[serial]
//     async fn test_websocket_send_receive() {
//         let server = setup_server().await;

//         let uri = "ws://127.0.0.1:8080".parse().unwrap();
//         let mut client = WebSocketClient::new(uri);
//         client.connect().await.unwrap();

//         // 注册客户端
//         client.register().await.unwrap();

//         // 发送消息
//         let payload = Payload::new_text(
//             Bytes::from("test_content".to_string()),
//             "test_id".to_string(),
//             Utc::now(),
//         );
//         client.send(payload.clone()).await.unwrap();

//         // 服务器应该能接收到消息
//         let received = server.subscribe().await.unwrap().unwrap();
//         assert_eq!(received, payload);
//     }

//     #[tokio::test]
//     #[serial]
//     async fn test_websocket_broadcast() {
//         let server = setup_server().await;

//         // 创建两个客户端
//         let uri: Uri = "ws://127.0.0.1:8080".parse().unwrap();
//         let mut client1 = WebSocketClient::new(uri.clone());
//         let mut client2 = WebSocketClient::new(uri);

//         client1.connect().await.unwrap();
//         client2.connect().await.unwrap();

//         client1.register().await.unwrap();
//         client2.register().await.unwrap();

//         // 广播消息
//         let payload = Payload::new_text(
//             Bytes::from("broadcast_content".to_string()),
//             "broadcast_id".to_string(),
//             Utc::now(),
//         );
//         server.broadcast(payload.clone(), None).await.unwrap();

//         // 两个客户端都应该收到消息
//         let received1 = client1.receive().await.unwrap();
//         let received2 = client2.receive().await.unwrap();

//         assert_eq!(received1, payload);
//         assert_eq!(received2, payload);
//     }
//     #[tokio::test]
//     #[serial]
//     async fn test_websocket_disconnect() {
//         let _ = setup_server().await;

//         let uri = "ws://127.0.0.1:8080".parse().unwrap();
//         let mut client = WebSocketClient::new(uri);
//         client.connect().await.unwrap();

//         client.disconnect().await.unwrap();

//         assert!(client.writer.is_none());
//         assert!(client.reader.is_none());
//     }
// }

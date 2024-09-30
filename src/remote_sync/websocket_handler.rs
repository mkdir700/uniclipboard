use super::traits::RemoteClipboardSync;
use crate::config::CONFIG;
use crate::device::get_device_manager;
use crate::{
    message::Payload,
    network::{WebSocketClient, WebSocketServer},
};
use anyhow::Result;
use async_trait::async_trait;
use log::info;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite::http::Uri;

pub struct WebSocketSync {
    is_server: bool,
    server_addr: Option<String>,
    server_port: Option<u16>,
    connect_server_addr: Option<String>,
    connect_server_port: Option<u16>,
    server: Option<Arc<WebSocketServer>>,
    client: Arc<RwLock<Option<WebSocketClient>>>,
}

impl WebSocketSync {
    pub fn new(is_server: bool) -> Self {
        let config = CONFIG.read().unwrap();
        Self {
            is_server,
            server_addr: config.websocket_server_addr.clone(),
            server_port: config.websocket_server_port,
            connect_server_addr: config.connect_websocket_server_addr.clone(),
            connect_server_port: config.connect_websocket_server_port,
            server: if is_server {
                Some(Arc::new(WebSocketServer::new()))
            } else {
                None
            },
            client: Arc::new(RwLock::new(None)),
        }
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

    async fn push(&self, payload: Payload) -> Result<()> {
        if self.is_server {
            let skip_addr: Option<String> = {
                let device_manager = get_device_manager().lock().unwrap();
                device_manager
                    .get_by_device_id(&payload.get_device_id())
                    .map(|device| {
                        format!(
                            "{}:{}",
                            device.ip.clone().unwrap(),
                            device.port.clone().unwrap()
                        )
                    })
            };

            // 对于 server 的push方法，就是将消息广播给其他客户端
            if let Some(server) = &self.server {
                // TODO: 这里需要修改，不能广播给所有，具体看 payload 是哪个客户端产生的
                server.broadcast(payload, skip_addr).await?;
            }
        } else {
            let client = self.client.read().await;
            if let Some(c) = client.as_ref() {
                c.send(payload).await?;
            } else {
                return Err(anyhow::anyhow!("Client not connected"));
            }
        }
        Ok(())
    }

    async fn pull(&self, timeout: Option<Duration>) -> Result<Payload> {
        let _ = timeout;
        if self.is_server {
            let server = self.server.as_ref().unwrap();
            if let Some(payload) = server.subscribe().await? {
                info!("Pull result: {}", payload);
                Ok(payload)
            } else {
                Err(anyhow::anyhow!("No payload received"))
            }
        } else {
            let result = {
                let client = self.client.read().await;
                if let Some(c) = client.as_ref() {
                    c.receive().await
                } else {
                    Err(anyhow::anyhow!("Client not connected"))
                }
            };

            match result {
                Ok(payload) => {
                    info!("Pull result: {}", payload);
                    Ok(payload)
                }
                Err(e) => {
                    let mut client = self.client.write().await;
                    if let Some(c) = client.as_mut() {
                        // 重新连接
                        c.connect().await?;
                        c.register().await?;
                    }
                    Err(e)
                }
            }
        }
    }

    async fn sync(&self) -> Result<()> {
        // 在这个简单的实现中，sync 可以是一个 no-op
        // 或者可以发送一个特殊的同步消息
        Ok(())
    }

    async fn start(&self) -> Result<()> {
        if self.is_server {
            if let Some(server) = &self.server {
                let server = server.clone();
                let server_addr = self.server_addr.clone().unwrap();
                let server_port = self.server_port.clone().unwrap();
                tokio::spawn(async move {
                    if let Err(e) = server
                        .run(format!("{}:{}", &server_addr, server_port).as_str())
                        .await
                    {
                        eprintln!("WebSocket server error: {:?}", e);
                    }
                });
            }
        } else {
            let mut client = self.client.write().await;
            let connect_server_addr = self.connect_server_addr.clone().unwrap();
            let connect_server_port = self.connect_server_port.clone().unwrap();
            let uri = format!("ws://{}:{}", connect_server_addr, connect_server_port)
                .parse::<Uri>()
                .unwrap();
            *client = Some(WebSocketClient::new(uri));
            if let Some(c) = client.as_mut() {
                c.connect().await?;
                // 向服务端发送注册消息
                c.register().await?;
            }
        }
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        if !self.is_server {
            let mut client = self.client.write().await;
            if let Some(c) = client.as_mut() {
                c.disconnect().await?;
            }
            *client = None;
        }
        Ok(())
    }
}

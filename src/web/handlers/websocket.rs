use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use futures::{FutureExt, StreamExt};
use log::{error, info};
use tokio::sync::mpsc;
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::{Message, WebSocket};

use super::websocket_message::MessageSource;
use super::websocket_message::WebSocketMessageHandler;

pub struct WebSocketHandler {
    message_handler: Arc<WebSocketMessageHandler>,
}

impl WebSocketHandler {
    pub fn new(websocket_message_handler: Arc<WebSocketMessageHandler>) -> Self {
        Self {
            message_handler: websocket_message_handler,
        }
    }

    pub async fn client_connected(&self, ws: WebSocket, addr: Option<SocketAddr>) {
        let (server_ws_sender, mut client_ws_rcv) = ws.split();
        let (client_sender, client_rcv) = tokio::sync::mpsc::unbounded_channel();
        let client_rcv = UnboundedReceiverStream::new(client_rcv);

        let client_id = match addr {
            Some(addr) => format!("{}:{}", addr.ip(), addr.port()),
            None => String::new(),
        };
        if client_id.is_empty() {
            error!("Client id is empty");
            return;
        }

        {
            self.message_handler
                .add_incoming_connection(client_id.clone(), client_sender)
                .await;

            let count = self.message_handler.count_incoming_connections().await;
            info!("Client {} connected, current clients: {}", client_id, count);
        }

        tokio::task::spawn(client_rcv.forward(server_ws_sender).map(|result| {
            if let Err(e) = result {
                error!("Error sending websocket msg to client: {}", e);
            }
        }));

        info!("Client [{}] connected", client_id);
        while let Some(result) = client_ws_rcv.next().await {
            let msg = match result {
                Ok(msg) => msg,
                Err(e) => {
                    error!("Error receiving ws message: {}", e);
                    break;
                }
            };
            if let Some(addr) = addr {
                self.message_handler
                    .handle_message(msg, MessageSource::IpPort(addr))
                    .await;
            } else {
                error!("Client [{}] connected, but addr is None", client_id);
            }
        }
        info!("Client [{}] disconnected", client_id);
        self.client_disconnected(client_id, addr).await;
    }

    async fn client_disconnected(&self, client_id: String, addr: Option<SocketAddr>) {
        // client_id 是 ip+port 的方式组合字符串
        self.message_handler
            .disconnect_incoming_connection(client_id, addr)
            .await;
    }
}

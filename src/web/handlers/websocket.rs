use std::net::SocketAddr;
use std::sync::Arc;

use futures::{FutureExt, StreamExt};
use log::{error, info};
use tokio_stream::wrappers::UnboundedReceiverStream;
use warp::ws::WebSocket;

use crate::connection::ConnectionManager;
use crate::web::handlers::message_handler::MessageSource;
use crate::web::handlers::websocket_message::WebSocketMessageHandler;

pub struct WebSocketHandler {
    message_handler: Arc<WebSocketMessageHandler>,
    connection_manager: Arc<ConnectionManager>,
}

impl WebSocketHandler {
    pub fn new(
        websocket_message_handler: Arc<WebSocketMessageHandler>,
        connection_manager: Arc<ConnectionManager>,
    ) -> Self {
        Self {
            message_handler: websocket_message_handler,
            connection_manager,
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
            self.connection_manager
                .incoming
                .add_connection(client_id.clone(), client_sender)
                .await;

            let count = self.connection_manager.incoming.count().await;
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
        // ? 如果客户端意外中止，client_disconnected 是否会一定被调用到？
        // ? If the client is terminated unexpectedly, will client_disconnected be called?
        if let Some(addr) = addr {
            self.client_disconnected(addr).await;
        }
    }

    async fn client_disconnected(&self, addr: SocketAddr) {
        info!("Client [{}] disconnected", addr);
        self.connection_manager
            .remove_connection(MessageSource::IpPort(addr))
            .await;
    }
}

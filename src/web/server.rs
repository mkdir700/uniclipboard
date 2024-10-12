use crate::web::routes::{device, download, websocket};
use anyhow::Result;
use std::{net::SocketAddr, sync::Arc};
use warp::Filter;

use super::{handle_rejection, handlers::websocket::WebSocketHandler};

// 定义 WebServer 结构体
pub struct WebServer {
    address: SocketAddr,
    websocket_handler: Arc<WebSocketHandler>,
}

impl WebServer {
    // 创建新的 WebServer 实例
    pub fn new(address: SocketAddr, websocket_handler: Arc<WebSocketHandler>) -> Self {
        Self {
            address,
            websocket_handler,
        }
    }

    // 启动 web 服务器的方法
    pub async fn run(&self) -> Result<()> {
        // API 路由
        let api_routes = warp::path("api").and(download::route().or(device::route()));
        // websocket 路由
        let websocket_routes = websocket::route(Arc::clone(&self.websocket_handler));

        // 合并路由
        let routes = api_routes.or(websocket_routes).recover(handle_rejection);

        // 启动服务器
        warp::serve(routes).run(self.address).await;

        Ok(())
    }
}

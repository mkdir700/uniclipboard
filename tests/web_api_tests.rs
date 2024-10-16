use bytes::Bytes;
use chrono::Utc;
use reqwest_dav::re_exports::reqwest::{self, StatusCode};
use serial_test::serial;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{self, time::timeout};

use uniclipboard::{
    device::Device,
    message::{ClipboardSyncMessage, DeviceListData, Payload, WebSocketMessage},
    network::WebSocketClient,
    web::{handlers::websocket_message::MessageSource, WebServer},
    Config, WebSocketHandler, WebSocketMessageHandler, CONFIG,
};

struct WebServerWrapper {
    websocket_message_handler: Arc<WebSocketMessageHandler>,
    #[allow(unused)]
    websocket_handler: Arc<WebSocketHandler>,
    webserver: Arc<WebServer>,
}

fn setup_config() -> Config {
    let mut config = CONFIG.write().unwrap();
    config.webserver_addr = Some("127.0.0.1".to_string());
    config.webserver_port = Some(8333);
    config.clone()
}

fn setup_webserver() -> WebServerWrapper {
    let config = setup_config();
    let websocket_message_handler = Arc::new(WebSocketMessageHandler::new());
    let websocket_handler = Arc::new(WebSocketHandler::new(websocket_message_handler.clone()));
    let webserver = WebServer::new(
        SocketAddr::new(
            config.webserver_addr.unwrap().parse().unwrap(),
            config.webserver_port.unwrap(),
        ),
        websocket_handler.clone(),
    );
    WebServerWrapper {
        websocket_message_handler: websocket_message_handler.clone(),
        websocket_handler: websocket_handler.clone(),
        webserver: Arc::new(webserver),
    }
}

#[tokio::test]
#[serial]
async fn test_404() {
    let w = setup_webserver();
    let webserver_clone = Arc::clone(&w.webserver);
    tokio::spawn(async move { webserver_clone.run().await });
    tokio::time::sleep(Duration::from_millis(500)).await;

    let response = reqwest::get("http://127.0.0.1:8333/api/device111")
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
#[serial]
async fn test_get_device_list() {
    let w = setup_webserver();
    let webserver_clone = Arc::clone(&w.webserver);
    tokio::spawn(async move { webserver_clone.run().await });
    tokio::time::sleep(Duration::from_millis(500)).await;

    // 第一次运行是空列表
    let response = reqwest::get("http://127.0.0.1:8333/api/device")
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.text().await.unwrap(),
        "{\"code\":200,\"message\":\"成功\",\"data\":{\"list\":[],\"total\":0}}"
    );

    // 添加一个设备
    let device = Device::new(
        "device1".to_string(),
        Some("127.0.0.1".to_string()),
        Some(8114),
        Some(8333),
    );
    // let data = DeviceListData {
    //     devices: vec![device],
    //     replay_device_ids: vec![],
    // };
    w.websocket_message_handler
        .handle_register(device)
        .await;

    // 再次运行
    let response = reqwest::get("http://127.0.0.1:8333/api/device")
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.text().await.unwrap(),
        "{\"code\":200,\"message\":\"成功\",\"data\":{\"list\":[{\"id\":\"device1\",\"ip\":\"127.0.0.1\",\"port\":8114,\"server_port\":8333}],\"total\":1}}"
    );
}

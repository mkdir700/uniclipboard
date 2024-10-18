use bytes::Bytes;
use chrono::Utc;
use serial_test::serial;
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tokio::{self, time::timeout};

use uniclipboard::{
    device::Device,
    message::{ClipboardSyncMessage, DeviceListData, Payload, WebSocketMessage},
    network::WebSocketClient,
    web::WebServer,
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
async fn test_websocket_run() {
    let w = setup_webserver();
    let webserver_clone = Arc::clone(&w.webserver);
    tokio::spawn(async move { webserver_clone.run().await });

    tokio::time::sleep(Duration::from_secs(1)).await;
    let mut client = WebSocketClient::new("ws://127.0.0.1:8333/ws".parse().unwrap());
    client.connect().await.unwrap();
    client.disconnect().await.unwrap();
    w.webserver.shutdown().await.unwrap();
}

#[tokio::test]
#[serial]
async fn test_websocket_broadcast() {
    let w = setup_webserver();
    let webserver_clone = Arc::clone(&w.webserver);
    tokio::spawn(async move { webserver_clone.run().await });
    tokio::time::sleep(Duration::from_secs(1)).await;

    let devices = vec![Device::new("1".to_string(), None, None, Some(8333))];

    let mut client1 = WebSocketClient::new("ws://127.0.0.1:8333/ws".parse().unwrap());
    client1.connect().await.unwrap();
    client1
        .register(Some(Device::new("1".to_string(), None, None, Some(8333))))
        .await
        .unwrap();

    let websocket_message_handler_clone = Arc::clone(&w.websocket_message_handler);
    websocket_message_handler_clone
        .broadcast(
            &WebSocketMessage::DeviceListSync(DeviceListData {
                devices,
                replay_device_ids: vec![],
            }),
            &None,
        )
        .await
        .unwrap();

    let handle1 = tokio::spawn(async move {
        let mut received_correct_message = false;
        for _ in 0..10 {
            // 尝试10次，防止无限循环
            match client1.receive_raw().await {
                Ok(message) => match message {
                    WebSocketMessage::DeviceListSync(data) => {
                        received_correct_message = true;
                        println!("收到 DeviceListSync 消息: {}", data.devices.len());
                        break;
                    }
                    _ => {}
                },
                Err(e) => {
                    println!("接收消息时出错: {}", e);
                    return Err(anyhow::anyhow!("接收消息失败"));
                }
            }
        }
        if received_correct_message {
            Ok(client1)
        } else {
            Err(anyhow::anyhow!("未收到预期的 DeviceListSync 消息"))
        }
    });

    // 等待 handle1 完成，设置5秒超时
    match timeout(Duration::from_secs(5), handle1).await {
        Ok(result) => {
            // handle1 已完成，检查结果
            match result {
                Ok(Ok(mut client)) => {
                    println!("测试成功完成");
                    client.disconnect().await.unwrap();
                }
                Ok(Err(e)) => panic!("测试失败: {}", e),
                Err(e) => panic!("任务执行出错: {}", e),
            }
        }
        Err(_) => panic!("测试超时"),
    }

    w.webserver.shutdown().await.unwrap();
}

/// 测试订阅消息
///
/// 创建一个 client，然后调用 subscribe 方法，查看是否有结果
#[tokio::test]
#[serial]
async fn test_websocket_subscribe() {
    let w = setup_webserver();
    let webserver_clone = Arc::clone(&w.webserver);
    tokio::spawn(async move { webserver_clone.run().await });
    tokio::time::sleep(Duration::from_secs(1)).await;

    let mut client1 = WebSocketClient::new("ws://127.0.0.1:8333/ws".parse().unwrap());
    client1.connect().await.unwrap();
    client1
        .register(Some(Device::new("1".to_string(), None, None, Some(8333))))
        .await
        .unwrap();

    let payload = Payload::new_text(Bytes::from("test"), String::from("1"), Utc::now());

    // client1 publish clipboard sync message
    client1
        .send_clipboard_sync(&WebSocketMessage::ClipboardSync(ClipboardSyncMessage {
            device_id: "1".to_string(),
            file_code: "1".to_string(),
            file_type: "text".to_string(),
            file_size: 4,
            payload: Some(payload),
            timestamp: Utc::now().timestamp_millis() as u64,
        }))
        .await
        .unwrap();

    let websocket_message_handler_clone = Arc::clone(&w.websocket_message_handler);
    let handle1 = tokio::spawn(async move {
        match websocket_message_handler_clone
            .subscribe_clipboard_sync()
            .await
        {
            Ok(Some(message)) => {
                println!("收到消息: {}", message);
                true
            }
            Ok(None) => {
                println!("未收到消息");
                false
            }
            Err(e) => panic!("订阅失败: {}", e),
        }
    });

    // 等待 handle1 完成，设置5秒超时
    match timeout(Duration::from_secs(5), handle1).await {
        Ok(result) => {
            // handle1 已完成，检查结果
            match result {
                Ok(true) => {
                    println!("测试成功完成");
                    assert!(true);
                }
                Ok(false) => {
                    println!("未收到消息");
                    assert!(false);
                }
                Err(e) => panic!("任务执行出错: {}", e),
            }
        }
        Err(_) => panic!("测试超时"),
    }

    client1.disconnect().await.unwrap();
    w.webserver.shutdown().await.unwrap();
}

// / 测试订阅设备上下线
// /
// / 调用 subscribe_device_online/subscribe_device_offline 方法
// /
// / 然后使用一个 client 调用 reigster
// /
// / 验证 websocket_handler_clone.subscribe 的结果
// #[tokio::test]
// #[serial]
// async fn test_websocket_subscribe_device_online_offline() {
//     let websocket_handler = Arc::new(WebSocketHandler::new());
//     let webserver = Arc::new(setup_webserver(websocket_handler.clone()));
//     let webserver_clone = Arc::clone(&webserver);
//     tokio::spawn(async move { webserver_clone.run().await });
//     tokio::time::sleep(Duration::from_secs(1)).await;

//     let websocket_handler_clone = Arc::clone(&websocket_handler);
//     let handle1 = tokio::spawn(async move {
//         match websocket_handler_clone.subscribe_device_online().await {
//             Ok(Some(message)) => {
//                 println!("收到消息: {}", message);
//                 true
//             }
//             Ok(None) => {
//                 println!("未收到消息");
//                 false
//             }
//             Err(e) => panic!("订阅失败: {}", e),
//         }
//     });

//     let websocket_handler_clone = Arc::clone(&websocket_handler);
//     let handle2 = tokio::spawn(async move {
//         match websocket_handler_clone.subscribe_device_offline().await {
//             Ok(Some(message)) => {
//                 println!("收到消息: {}", message);
//                 true
//             }
//             Ok(None) => {
//                 println!("未收到消息");
//                 false
//             }
//             Err(e) => panic!("订阅失败: {}", e),
//         }
//     });

//     let mut client1 = WebSocketClient::new("ws://127.0.0.1:8333/ws".parse().unwrap());
//     client1.connect().await.unwrap();
//     client1
//         .register(Some(Device::new("1".to_string(), None, None, Some(8333))))
//         .await
//         .unwrap();

//     // 等待 handle1 完成，设置5秒超时
//     match timeout(Duration::from_secs(3), handle1).await {
//         Ok(result) => {
//             // handle1 已完成，检查结果
//             match result {
//                 Ok(true) => {
//                     println!("测试成功完成");
//                     assert!(true);
//                 }
//                 Ok(false) => {
//                     println!("未收到消息");
//                     assert!(false);
//                 }
//                 Err(e) => panic!("任务执行出错: {}", e),
//             }
//         }
//         Err(_) => panic!("测试超时"),
//     }

//     client1.disconnect().await.unwrap();

//     match timeout(Duration::from_secs(3), handle2).await {
//         Ok(result) => {
//             // handle2 已完成，检查结果
//             match result {
//                 Ok(true) => {
//                     println!("测试成功完成");
//                     assert!(true);
//                 }
//                 Ok(false) => {
//                     println!("未收到消息");
//                     assert!(false);
//                 }
//                 Err(e) => panic!("任务执行出错: {}", e),
//             }
//         }
//         Err(_) => panic!("测试超时"),
//     }

//     webserver.shutdown().await.unwrap();
// }

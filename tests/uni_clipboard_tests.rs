use anyhow::Result;
use bytes::Bytes;
use chrono::Utc;
use serial_test::serial;
use std::{sync::Arc, time::Duration};
use uniclipboard::{Payload, WebSocketHandler};
use uniclipboard::{
    clipboard::LocalClipboard,
    config::CONFIG,
    remote_sync::{RemoteSyncManager, WebSocketSync},
    uni_clipboard::{UniClipboard, UniClipboardBuilder},
    RemoteSyncManagerTrait,
};

fn setup_config() {
    let mut config = CONFIG.write().unwrap();
    config.webserver_addr = Some("127.0.0.1".to_string());
    config.webserver_port = Some(8333);
    config.connect_websocket_server_addr = Some("127.0.0.1".to_string());
    config.connect_websocket_server_port = Some(8333);
}

// 辅助函数：创建测试用的 UniClipboard 实例
async fn create_test_uni_clipboard() -> Result<UniClipboard> {
    let local_clipboard = Arc::new(LocalClipboard::new());
    let remote_sync_manager = Arc::new(RemoteSyncManager::new());
    let websocket_handler = Arc::new(WebSocketHandler::new());
    let websocket_sync = Arc::new(WebSocketSync::new(websocket_handler));

    remote_sync_manager.set_sync_handler(websocket_sync).await;

    UniClipboardBuilder::new()
        .set_local_clipboard(local_clipboard)
        .set_remote_sync(remote_sync_manager)
        .build()
}

#[tokio::test]
#[cfg_attr(not(feature = "integration_tests"), ignore)]
#[serial]
async fn test_uni_clipboard_start_stop() -> Result<()> {
    setup_config();
    let uni_clipboard = create_test_uni_clipboard().await?;

    assert!(uni_clipboard.start().await.is_ok(), "启动 UniClipboard 失败");
    assert!(uni_clipboard.start().await.is_err(), "重复启动 UniClipboard 应该失败");
    assert!(uni_clipboard.stop().await.is_ok(), "停止 UniClipboard 失败");
    assert!(uni_clipboard.stop().await.is_err(), "重复停止 UniClipboard 应该失败");

    Ok(())
}

#[tokio::test]
#[cfg_attr(not(feature = "integration_tests"), ignore)]
#[serial]
async fn test_uni_clipboard_pause_resume() -> Result<()> {
    setup_config();
    let uni_clipboard = create_test_uni_clipboard().await?;

    assert!(uni_clipboard.start().await.is_ok(), "启动 UniClipboard 失败");
    assert!(uni_clipboard.pause().await.is_ok(), "暂停 UniClipboard 失败");
    assert!(uni_clipboard.resume().await.is_ok(), "恢复 UniClipboard 失败");
    assert!(uni_clipboard.stop().await.is_ok(), "停止 UniClipboard 失败");

    Ok(())
}

#[tokio::test]
#[cfg_attr(not(feature = "integration_tests"), ignore)]
#[serial]
async fn test_uni_clipboard_client_server_sync() -> Result<()> {
    setup_config();
    // 创建服务器和客户端实例
    let server = create_test_uni_clipboard().await?;
    let client = create_test_uni_clipboard().await?;

    // 启动服务器和客户端
    server.start().await?;
    // 等待连接建立
    tokio::time::sleep(Duration::from_secs(1)).await;
    client.start().await?;

    let client_clipboard = client.get_clipboard();
    let test_payload = Payload::new_text(Bytes::from("test"), "device_id".to_string(), Utc::now());
    // 从客户端写入一条消息，然后从服务器读取
    client_clipboard.write(test_payload.clone()).await?;
    
    let content = server.get_clipboard().read().await?;
    assert_eq!(content, test_payload, "客户端到服务器的同步失败");

    // 停止服务器和客户端
    server.stop().await?;
    client.stop().await?;

    Ok(())
}

#[tokio::test]
#[cfg_attr(not(feature = "integration_tests"), ignore)]
#[serial]
async fn test_uni_clipboard_server_to_client_sync() -> Result<()> {
    setup_config();
    // 创建服务器和客户端实例
    let server = create_test_uni_clipboard().await?;
    let client = create_test_uni_clipboard().await?;

    // 启动服务器和客户端
    server.start().await?;
    // 等待连接建立
    tokio::time::sleep(Duration::from_secs(1)).await;
    client.start().await?;

    let test_payload = Payload::new_text(Bytes::from("test"), "device_id".to_string(), Utc::now());
    // 从服务器写入一条消息，然后从客户端读取
    server.get_clipboard().write(test_payload.clone()).await?;
    
    let content = client.get_clipboard().read().await?;
    assert_eq!(content, test_payload, "服务器到客户端的同步失败");

    // 停止服务器和客户端
    server.stop().await?;
    client.stop().await?;

    Ok(())
}

#[tokio::test]
#[cfg_attr(not(feature = "integration_tests"), ignore)]
#[serial]
async fn test_uni_clipboard_duplicate_content_handling() -> Result<()> {
    setup_config();
    let uni_clipboard = create_test_uni_clipboard().await?;
    uni_clipboard.start().await?;

    let clipboard = uni_clipboard.get_clipboard();
    let test_payload = Payload::new_text(Bytes::from("Duplicate content"), "device_id".to_string(), Utc::now());

    // 写入内容
    clipboard.write(test_payload.clone()).await?;
    
    // 等待同步
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 再次写入相同的内容
    clipboard.write(test_payload.clone()).await?;

    // 等待同步
    tokio::time::sleep(Duration::from_secs(1)).await;

    // 读取内容，应该与最初写入的内容相同
    let content = clipboard.read().await?;
    assert_eq!(content, test_payload, "重复内容处理失败");

    uni_clipboard.stop().await?;

    Ok(())
}


use anyhow::Result;
use bytes::Bytes;
use chrono::Utc;
use dotenv::dotenv;
use std::env;
use std::sync::Once;
use std::time::Duration;
use uniclipboard::{
    clipboard::CloudClipboardHandler,
    config::{Config, CONFIG},
    message::Payload,
    network::WebDAVClient,
};

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        dotenv().ok();
        let test_config = Config {
            device_id: "test-device".to_string(),
            webdav_url: env::var("WEBDAV_URL").expect("WEBDAV_URL not set"),
            username: env::var("WEBDAV_USERNAME").expect("WEBDAV_USERNAME not set"),
            password: env::var("WEBDAV_PASSWORD").expect("WEBDAV_PASSWORD not set"),
            push_interval: Some(500),
            pull_interval: Some(500),
            sync_interval: Some(500),
            enable_push: Some(true),
            enable_pull: Some(true),
        };
        *CONFIG.write().unwrap() = test_config;
    });
}

async fn create_webdav_client() -> Result<WebDAVClient> {
    setup();
    let config = CONFIG.read().unwrap();
    WebDAVClient::new(
        config.get_webdav_url(),
        config.get_username(),
        config.get_password(),
    )
    .await
}

#[tokio::test]
async fn test_cloud_clipboard_push_text() {
    setup();
    let client = create_webdav_client().await.unwrap();
    let handler = CloudClipboardHandler::new(client);
    let payload = Payload::new_text(
        Bytes::from("test content"),
        "test_device".to_string(),
        Utc::now(),
    );

    let result = handler.push(payload).await;
    assert!(result.is_ok());
    let path = result.unwrap();
    let client_clone = handler.get_client();
    let meta = client_clone
        .fetch_latest_file_meta(handler.base_path.clone())
        .await
        .unwrap();
    assert_eq!(meta.get_path(), path);

    client_clone.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_cloud_clipboard_push_image() {
    setup();
    let client = create_webdav_client().await.unwrap();
    let handler = CloudClipboardHandler::new(client);
    let payload = Payload::new_image(
        Bytes::from(vec![0u8; 100]), // 模拟图片数据
        "test_device".to_string(),
        Utc::now(),
        100,
        100,
        "png".to_string(),
        100,
    );

    let result = handler.push(payload).await;
    assert!(result.is_ok());
    let path = result.unwrap();
    let client_clone = handler.get_client();
    let meta = client_clone
        .fetch_latest_file_meta(handler.base_path.clone())
        .await
        .unwrap();
    assert_eq!(meta.get_path(), path);

    client_clone.delete(path).await.unwrap();
}

#[tokio::test]
async fn test_cloud_clipboard_pull_text() {
    setup();
    let client = create_webdav_client().await.unwrap();
    let handler = CloudClipboardHandler::new(client);

    // push a text file to the cloud clipboard
    let payload = Payload::new_text(
        Bytes::from("test content"),
        "device-1".to_string(),
        Utc::now(),
    );

    let result = handler.push(payload).await;
    assert!(result.is_ok());
    let _path = result.unwrap();

    // pull the file from the cloud clipboard
    let result = handler.pull(Some(Duration::from_secs(1))).await;
    if let Err(e) = &result {
        println!("Pull error: {:?}", e);
    }
    assert!(result.is_ok());
    let pulled_payload = result.unwrap();
    assert!(matches!(pulled_payload, Payload::Text(_)));
    if let Payload::Text(text_payload) = pulled_payload {
        assert_eq!(*text_payload.content, Bytes::from("test content"));
    }
}

#[tokio::test]
async fn test_cloud_clipboard_pull_image() {
    setup();
    let client = create_webdav_client().await.unwrap();
    let handler = CloudClipboardHandler::new(client);

    // push an image file to the cloud clipboard
    let image_data = vec![0u8; 100]; // 模拟图片数据
    let payload = Payload::new_image(
        Bytes::from(image_data.clone()),
        "device-1".to_string(),
        Utc::now(),
        100,
        100,
        "png".to_string(),
        100,
    );

    let result = handler.push(payload).await;
    assert!(result.is_ok());
    let _path = result.unwrap();

    // pull the file from the cloud clipboard
    let result = handler.pull(Some(Duration::from_secs(1))).await;
    if let Err(e) = &result {
        println!("Pull error: {:?}", e);
    }
    assert!(result.is_ok());
    let pulled_payload = result.unwrap();
    assert!(matches!(pulled_payload, Payload::Image(_)));
    if let Payload::Image(image_payload) = pulled_payload {
        assert_eq!(*image_payload.content, Bytes::from(image_data));
        assert_eq!(image_payload.width, 100);
        assert_eq!(image_payload.height, 100);
        assert_eq!(image_payload.format, "png");
        assert_eq!(image_payload.size, 100);
    }
}

// #[test]
// fn test_local_clipboard_text() {
//     let handler = LocalClipboardHandler::new();
    
//     // Write text to clipboard
//     let text_payload = Payload::new_text(
//         Bytes::from("test content"),
//         "test_device".to_string(),
//         Utc::now(),
//     );
//     assert!(handler.write(text_payload.clone()).is_ok());

//     // Read from clipboard
//     let result = handler.read();
//     assert!(result.is_ok());
//     let read_payload = result.unwrap();
//     assert!(matches!(read_payload, Payload::Text(_)));
//     if let Payload::Text(text_payload) = read_payload {
//         assert_eq!(*text_payload.content, Bytes::from("test content"));
//     }
// }

// #[test]
// fn test_local_clipboard_image() {
//     let handler = LocalClipboardHandler::new();
    
//     // Write image to clipboard
//     let image_data = vec![0u8; 100]; // 模拟图片数据
//     let image_payload = Payload::new_image(
//         Bytes::from(image_data.clone()),
//         "test_device".to_string(),
//         Utc::now(),
//         100,
//         100,
//         "png".to_string(),
//         100,
//     );
//     assert!(handler.write(image_payload.clone()).is_ok());

//     // Read from clipboard
//     let result = handler.read();
//     assert!(result.is_ok());
//     let read_payload = result.unwrap();
//     assert!(matches!(read_payload, Payload::Image(_)));
//     if let Payload::Image(image_payload) = read_payload {
//         assert_eq!(*image_payload.content, Bytes::from(image_data));
//         assert_eq!(image_payload.width, 100);
//         assert_eq!(image_payload.height, 100);
//         assert_eq!(image_payload.format, "png");
//         assert_eq!(image_payload.size, 100);
//     }
// }

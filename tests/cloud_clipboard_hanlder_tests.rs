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
async fn test_cloud_clipboard_push() {
    setup();
    let client = create_webdav_client().await.unwrap();
    let handler = CloudClipboardHandler::new(client);
    let payload = Payload::new(
        Bytes::from("test content"),
        "text".to_string(),
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
async fn test_cloud_clipboard_pull() {
    setup();
    let client = create_webdav_client().await.unwrap();
    let handler = CloudClipboardHandler::new(client);

    // push a file to the cloud clipboard
    let payload = Payload::new(
        Bytes::from("test content"),
        "text".to_string(),
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
    let payload = result.unwrap();
    assert_eq!(payload.content, Bytes::from("test content"));
}

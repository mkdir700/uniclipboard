use anyhow::Result;
use bytes::Bytes;
use chrono::Utc;
use dotenv::dotenv;
use std::env;
use uniclipboard::{
    clipboard::CloudClipboardHandler, message::Payload,
    network::WebDAVClient,
};

fn load_env() {
    dotenv().ok();
}

async fn create_webdav_client() -> Result<WebDAVClient> {
    load_env();
    let webdav_url = env::var("WEBDAV_URL").expect("WEBDAV_URL not set");
    let username = env::var("WEBDAV_USERNAME").expect("WEBDAV_USERNAME not set");
    let password = env::var("WEBDAV_PASSWORD").expect("WEBDAV_PASSWORD not set");

    WebDAVClient::new(webdav_url, username, password).await
}

#[tokio::test]
async fn test_cloud_clipboard_push() {
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
}

#[tokio::test]
async fn test_cloud_clipboard_pull() {
    let client = create_webdav_client().await.unwrap();
    let handler = CloudClipboardHandler::new(client);

    let result = handler.pull().await;
    assert!(result.is_ok());
    let payload = result.unwrap();
    assert!(!payload.content.is_empty());
}

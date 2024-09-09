use anyhow::Result;
use bytes::Bytes;
use chrono::Utc;
use dotenv::dotenv;
use std::env;
use uniclipboard::{Payload, WebDAVClient};

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
async fn test_webdav_client_upload_and_download() -> Result<()> {
    let client = create_webdav_client().await?;

    let test_content = "测试内容".as_bytes().to_vec();
    let payload = Payload {
        device_id: "test_device".to_string(),
        content_type: "text/plain".to_string(),
        content: Bytes::from(test_content.clone()),
        timestamp: Utc::now(),
    };

    // 上传文件
    let upload_path = "/test_upload.txt".to_string();
    let filepath = client.upload(upload_path.clone(), payload).await?;

    // 下载文件
    let downloaded_payload = client.download(filepath.clone()).await?;

    // 验证下载的内容
    assert_eq!(downloaded_payload.device_id, "test_device");
    assert_eq!(downloaded_payload.content_type, "text/plain");
    assert_eq!(downloaded_payload.content, test_content);

    // 删除文件
    client.delete_file(filepath.clone()).await?;
    Ok(())
}

#[tokio::test]
async fn test_webdav_client_get_latest_added_file() -> Result<()> {
    let client = create_webdav_client().await?;

    // 上传两个文件
    let payload1 = Payload::new(
        Bytes::from("文件1"),
        "text/plain".to_string(),
        "test_device".to_string(),
        Utc::now(),
    );
    let file1_path = client
        .upload("/".to_string(), payload1)
        .await?;
    
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await; // 确保时间戳不同

    let payload2 = Payload::new(
        Bytes::from("文件2"),
        "text/plain".to_string(),
        "test_device".to_string(),
        Utc::now(),
    );
    let file2_path = client
        .upload("/".to_string(), payload2)
        .await?;

    // 获取最新添加的文件
    let latest_file_meta = client.fetch_latest_file_meta("/".to_string()).await?;

    assert_eq!(latest_file_meta.get_path(), file2_path);

    // 清理测试文件
    client.delete_file(file1_path).await?;
    client.delete_file(file2_path).await?;

    Ok(())
}

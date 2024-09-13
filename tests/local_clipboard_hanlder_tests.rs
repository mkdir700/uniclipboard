use bytes::Bytes;
use chrono::Utc;
use image::ImageReader;
use lazy_static::lazy_static;
use std::fs;
use std::io::Write;
use std::sync::Mutex;
use std::{fs::File, path::PathBuf};
use uniclipboard::{LocalClipboardHandler, Payload};

lazy_static! {
    static ref CLIPBOARD_MUTEX: Mutex<()> = Mutex::new(());
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "该测试用例仅在本地测试环境下有效，在 CI/CD 环境下会报错，因为无法访问本地剪贴板"]
async fn test_read_image_from_local_clipboard() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let handler = LocalClipboardHandler::new();

    let payload = handler.read().await.unwrap();
    if let Payload::Image(image_payload) = payload {
        println!("size: {} MB", image_payload.size as f32 / 1024.0 / 1024.0);
        let mut file = File::create("test_image.png").unwrap();
        file.write_all(&image_payload.content).unwrap();
    } else {
        panic!("读取到的 payload 不是图像");
    }
}

#[tokio::test]
#[ignore = "该测试用例仅在本地测试环境下有效，在 CI/CD 环境下会报错，因为无法访问本地剪贴板"]
async fn test_write_image_to_local_clipboard() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 读取测试图片
    let image_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_resources")
        .join("google.png");

    let img = ImageReader::open(&image_path)?.decode()?;
    let (width, height) = (img.width(), img.height());
    let image_bytes = fs::read(&image_path)?;

    let payload = Payload::new_image(
        Bytes::from(image_bytes.clone()),
        "test_device".to_string(),
        Utc::now(),
        width as usize,
        height as usize,
        "png".to_string(),
        image_bytes.len(),
    );
    let local_handler = LocalClipboardHandler::new();
    local_handler.write(payload.clone()).await?;

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    Ok(())
}

#[tokio::test]
#[ignore = "该测试用例仅在本地测试环境下有效，在 CI/CD 环境下会报错，因为无法访问本地剪贴板"]
async fn test_read_write_clipboard_text() {
    let test_text = "Hello, world!";
    let local_handler = LocalClipboardHandler::new();
    let payload = local_handler.read().await.unwrap();
    if let Payload::Text(text_payload) = payload {
        assert_ne!(text_payload.text(), test_text);
    } else {
        panic!("读取到的 payload 不是文本");
    }

    let write_payload = Payload::new_text(
        Bytes::from(test_text.to_string()),
        "test_device".to_string(),
        Utc::now(),
    );
    local_handler.write(write_payload).await.unwrap();
    let payload = local_handler.read().await.unwrap();
    if let Payload::Text(text_payload) = payload {
        assert_eq!(text_payload.text(), test_text);
    } else {
        panic!("读取到的 payload 不是文本");
    }
}

#[tokio::test]
#[ignore = "该测试用例仅在本地测试环境下有效，在 CI/CD 环境下会报错，因为无法访问本地剪贴板"]
async fn test_read_write_clipboard_image() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 读取测试图片
    let image_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_resources")
        .join("2048.jpeg");

    let img = ImageReader::open(&image_path)?.decode()?;
    let (width, height) = (img.width(), img.height());
    let image_bytes = fs::read(&image_path)?;

    // 创建 Payload
    let payload = Payload::new_image(
        Bytes::from(image_bytes.clone()),
        "test_device".to_string(),
        Utc::now(),
        width as usize,
        height as usize,
        "jpeg".to_string(),
        image_bytes.len(),
    );
    let local_handler = LocalClipboardHandler::new();
    local_handler.write(payload).await?;
    Ok(())
}

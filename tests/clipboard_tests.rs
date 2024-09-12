use arboard::{Clipboard, ImageData};
use bytes::Bytes;
use chrono::Utc;
use image::{DynamicImage, ImageReader};
use lazy_static::lazy_static;
use std::fs;
use std::io::Write;
use std::sync::Mutex;
use std::time::Duration;
use std::{fs::File, path::PathBuf};
use tokio::time::timeout;
use uniclipboard::{gen_from_img, LocalClipboardHandler, Payload};

lazy_static! {
    static ref CLIPBOARD_MUTEX: Mutex<()> = Mutex::new(());
}

#[tokio::test]
async fn test_local_clipboard_write_and_read() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let handler = LocalClipboardHandler::new();

    // 准备测试数据
    let test_content = "测试剪贴板内容";
    let payload = Payload::new_text(Bytes::from(test_content), "local".to_string(), Utc::now());
    // 测试写入
    assert!(handler.write(payload.clone()).await.is_ok());

    // 测试读取
    let read_result = handler.read().await;
    assert!(read_result.is_ok());

    let read_payload = read_result.unwrap();
    assert_eq!(read_payload.get_content(), payload.get_content());
    // FIXEME: 因为没有读取全局配置，所以 device_id 拿到的是空字符串
    // assert_eq!(read_payload.get_device_id(), payload.get_device_id());
}

#[tokio::test]
async fn test_local_clipboard_pull() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let handler = LocalClipboardHandler::new();

    // 准备初始测试数据
    let initial_content = "初始剪贴板内容";
    let initial_payload = Payload::new_text(
        Bytes::from(initial_content),
        "local".to_string(),
        Utc::now(),
    );
    handler.write(initial_payload).await.unwrap();

    // 在另一个任务中更改剪贴板内容
    tokio::spawn(async move {
        let handler_clone = LocalClipboardHandler::new();
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        let new_content = "新的剪贴板内容";
        let new_payload =
            Payload::new_text(Bytes::from(new_content), "local".to_string(), Utc::now());
        handler_clone.write(new_payload).await.unwrap();
    });

    // 测试 pull 函数
    let pulled_payload = handler
        .pull(Some(Duration::from_millis(400)))
        .await
        .unwrap();

    assert_eq!(*pulled_payload.get_content(), Bytes::from("新的剪贴板内容"));
    // assert_eq!(pulled_payload.get_device_id(), "local");
}

#[tokio::test]
async fn test_local_clipboard_pull_no_change() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let handler = LocalClipboardHandler::new();

    // 设置初始内容
    let content = "不变的剪贴板内容";
    let payload = Payload::new_text(Bytes::from(content), "local".to_string(), Utc::now());
    handler.write(payload).await.unwrap();

    // 使用 timeout 来测试 pull 操作是否会超时
    let pull_result = timeout(
        Duration::from_millis(400),
        handler.pull(Some(Duration::from_millis(300))),
    )
    .await;

    assert!(pull_result.is_err(), "预期 pull 操作超时，但并未发生");
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

// 测试读写剪切板，在 windows 上
#[test]
#[ignore = "该测试用例仅在本地测试环境下有效，在 CI/CD 环境下会报错，因为无法访问本地剪贴板"]
fn test_read_write_clipboard() {
    let mut clipboard = Clipboard::new().unwrap();
    clipboard
        .set_text("Hello, world11111111111!".to_string())
        .unwrap();
    let text = clipboard.get_text().unwrap();
    assert_eq!(text, "Hello, world11111111111!");
}

#[test]
#[ignore = "该测试用例仅在本地测试环境下有效，在 CI/CD 环境下会报错，因为无法访问本地剪贴板"]
fn test_read_write_clipboard_image() -> Result<(), Box<dyn std::error::Error>> {
    let mut clipboard = Clipboard::new().unwrap();
    // 1. 读取测试图片
    let image_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("test_resources")
        .join("google.png");

    let img = ImageReader::open(&image_path)?.decode()?;
    let (width, height) = (img.width(), img.height());
    let image_bytes = fs::read(&image_path)?;
    let dib = gen_from_img(&img);
    let payload = Payload::new_image(
        Bytes::from(dib),
        "test_device".to_string(),
        Utc::now(),
        width as usize,
        height as usize,
        "png".to_string(),
        image_bytes.len(),
    );
    LocalClipboardHandler::write_to_clipboard(&mut clipboard, payload)?;
    Ok(())
}

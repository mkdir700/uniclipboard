use bytes::Bytes;
use chrono::Utc;
use serial_test::serial;
use std::time::Duration;
use tokio::time::timeout;
use uniclipboard::{LocalClipboard, Payload};

#[tokio::test]
#[cfg_attr(not(feature = "clipboard_tests"), ignore)]
#[serial]
async fn test_local_clipboard_write_and_read() {
    let handler = LocalClipboard::new();

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
#[cfg_attr(not(feature = "clipboard_tests"), ignore)]
#[serial]
async fn test_local_clipboard_pull() {
    let handler = LocalClipboard::new();

    // 准备初始测试数据
    let initial_content = "123";
    let initial_payload = Payload::new_text(
        Bytes::from(initial_content),
        "local".to_string(),
        Utc::now(),
    );
    handler.write(initial_payload).await.unwrap();

    // 在另一个任务中更改剪贴板内容
    tokio::spawn(async move {
        let handler_clone = LocalClipboard::new();
        let new_content = "456";
        let new_payload =
            Payload::new_text(Bytes::from(new_content), "local".to_string(), Utc::now());
        handler_clone.write(new_payload).await.unwrap();
        println!("new_content: {}", new_content);
    });

    // 等待 100ms 后，确保新的内容已经写入
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // 测试 pull 函数
    let pulled_payload = handler
        .pull(Some(Duration::from_millis(400)))
        .await
        .unwrap();

    assert_eq!(*pulled_payload.get_content(), Bytes::from("456"));
    // assert_eq!(pulled_payload.get_device_id(), "local");
}

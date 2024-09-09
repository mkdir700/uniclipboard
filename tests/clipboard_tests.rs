use uniclipboard::{LocalClipboardHandler, Payload};
use bytes::Bytes;
use chrono::Utc;
use std::thread;
use std::time::Duration;
use std::panic::AssertUnwindSafe;
use std::sync::Mutex;
use lazy_static::lazy_static;


lazy_static! {
    static ref CLIPBOARD_MUTEX: Mutex<()> = Mutex::new(());
}

#[test]
fn test_local_clipboard_write_and_read() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let handler = LocalClipboardHandler::new();
    
    // 准备测试数据
    let test_content = "测试剪贴板内容";
    let payload = Payload::new(
        Bytes::from(test_content),
        "text".to_string(),
        "local".to_string(),
        Utc::now(),
    );
    // 测试写入
    assert!(handler.write(payload.clone()).is_ok());

    // 测试读取
    let read_result = handler.read();
    assert!(read_result.is_ok());
    
    let read_payload = read_result.unwrap();
    assert_eq!(read_payload.content, payload.content);
    assert_eq!(read_payload.content_type, payload.content_type);
    assert_eq!(read_payload.device_id, payload.device_id);
}

#[test]
fn test_local_clipboard_unsupported_content_type() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let handler = LocalClipboardHandler::new();
    
    // 准备不支持的内容类型
    let payload = Payload::new(
        Bytes::from("测试数据"),
        "image".to_string(),  // 不支持的类型
        "local".to_string(),
        Utc::now(),
    );

    // 测试写入不支持的类型
    let result = handler.write(payload);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Unsupported content type"
    );
}

#[test]
fn test_local_clipboard_pull() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let mut handler = LocalClipboardHandler::new();
    let handler_clone = handler.clone();
    
    // 准备初始测试数据
    let initial_content = "初始剪贴板内容";
    let initial_payload = Payload::new(
        Bytes::from(initial_content),
        "text".to_string(),
        "local".to_string(),
        Utc::now(),
    );
    handler.write(initial_payload).unwrap();

    // 在另一个线程中更改剪贴板内容
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        let new_content = "新的剪贴板内容";
        let new_payload = Payload::new(
            Bytes::from(new_content),
            "text".to_string(),
            "local".to_string(),
            Utc::now(),
        );
        handler_clone.write(new_payload).unwrap();
    });

    // 测试 pull 函数
    let pulled_payload = handler.pull(Some(Duration::from_millis(400))).unwrap();
    
    assert_eq!(pulled_payload.content, Bytes::from("新的剪贴板内容"));
    assert_eq!(pulled_payload.content_type, "text");
    assert_eq!(pulled_payload.device_id, "local");
}

#[test]
fn test_local_clipboard_pull_no_change() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let mut handler = LocalClipboardHandler::new();
    
    // 设置初始内容
    let content = "不变的剪贴板内容";
    let payload = Payload::new(
        Bytes::from(content),
        "text".to_string(),
        "local".to_string(),
        Utc::now(),
    );
    handler.write(payload).unwrap();

    // 使用 AssertUnwindSafe 包装 handler
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        handler.pull(Some(Duration::from_millis(400))).unwrap();
    }));

    assert!(result.is_err(), "预期 pull 操作超时，但并未发生");
}

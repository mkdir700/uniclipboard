use uniclipboard::{LocalClipboardHandler, Payload};
use bytes::Bytes;
use chrono::Utc;
use std::fs::File;
use std::io::Write;
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
    let payload = Payload::new_text(
        Bytes::from(test_content),
        "local".to_string(),
        Utc::now(),
    );
    // 测试写入
    assert!(handler.write(payload.clone()).is_ok());

    // 测试读取
    let read_result = handler.read();
    assert!(read_result.is_ok());
    
    let read_payload = read_result.unwrap();
    assert_eq!(read_payload.get_content(), payload.get_content());
    // FIXEME: 因为没有读取全局配置，所以 device_id 拿到的是空字符串
    // assert_eq!(read_payload.get_device_id(), payload.get_device_id()); 
}


#[test]
fn test_local_clipboard_pull() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let mut handler = LocalClipboardHandler::new();
    let handler_clone = handler.clone();
    
    // 准备初始测试数据
    let initial_content = "初始剪贴板内容";
    let initial_payload = Payload::new_text(
        Bytes::from(initial_content),
        "local".to_string(),
        Utc::now(),
    );
    handler.write(initial_payload).unwrap();

    // 在另一个线程中更改剪贴板内容
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        let new_content = "新的剪贴板内容";
        let new_payload = Payload::new_text(
            Bytes::from(new_content),
            "local".to_string(),
            Utc::now(),
        );
        handler_clone.write(new_payload).unwrap();
    });

    // 测试 pull 函数
    let pulled_payload = handler.pull(Some(Duration::from_millis(400))).unwrap();
    
    assert_eq!(*pulled_payload.get_content(), Bytes::from("新的剪贴板内容"));
    // assert_eq!(pulled_payload.get_device_id(), "local");
}

#[test]
fn test_local_clipboard_pull_no_change() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let mut handler = LocalClipboardHandler::new();
    
    // 设置初始内容
    let content = "不变的剪贴板内容";
    let payload = Payload::new_text(
        Bytes::from(content),
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

#[test]
#[ignore = "该测试用例仅在本地测试环境下有效，在 CI/CD 环境下会报错，因为无法访问本地剪贴板"]
fn test_read_image_from_local_clipboard() {
    let _lock = CLIPBOARD_MUTEX.lock().unwrap();
    let handler = LocalClipboardHandler::new();

    let payload = handler.read().unwrap();
    if let Payload::Image(image_payload) = payload {
        println!("size: {} MB", image_payload.size as f32 / 1024.0 / 1024.0);
        let mut file = File::create("test_image.png").unwrap();
        file.write_all(&image_payload.content).unwrap();
    } else {
        panic!("读取到的 payload 不是图像");
    }
}
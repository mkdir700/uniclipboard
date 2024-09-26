use anyhow::Result;
use dotenv::dotenv;
use std::env;
use std::sync::{Arc, Once};
use uniclipboard::{
    config::{Config, CONFIG},
    network::WebDAVClient,
    uni_clipboard::UniClipboardBuilder,
};

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        dotenv().ok();
        let mut test_config = Config::default();
        test_config.device_id = "test-device".to_string();
        test_config.webdav_url = Some(env::var("WEBDAV_URL").expect("WEBDAV_URL not set"));
        test_config.username = Some(env::var("WEBDAV_USERNAME").expect("WEBDAV_USERNAME not set"));
        test_config.password = Some(env::var("WEBDAV_PASSWORD").expect("WEBDAV_PASSWORD not set"));
        *CONFIG.write().unwrap() = test_config;
    });
}

async fn create_webdav_client() -> Result<WebDAVClient> {
    setup();
    let webdav_url = env::var("WEBDAV_URL").expect("WEBDAV_URL not set");
    let username = env::var("WEBDAV_USERNAME").expect("WEBDAV_USERNAME not set");
    let password = env::var("WEBDAV_PASSWORD").expect("WEBDAV_PASSWORD not set");

    WebDAVClient::new(webdav_url, username, password).await
}

// TODO: 等待实现 MockWebDAVClient 再进行测试
// #[tokio::test]
// async fn test_uni_clipboard_new_without_key_mouse_monitor() {
//     CONFIG.write().unwrap().enable_key_mouse_monitor = Some(false);
//     let webdav_client = MockWebDAVClient;
//     let uni_clipboard = UniClipboard::new(webdav_client);

//     assert!(uni_clipboard.key_mouse_monitor.is_none());
//     // 其他断言...
// }

// #[tokio::test]
// #[cfg(feature = "testing")]
// #[cfg_attr(not(feature = "hardware_tests"), ignore)]
// async fn test_uni_clipboard_sleep_wake_cycle() {
//     use uniclipboard::LocalClipboard;

//     let webdav_client = create_webdav_client().await.unwrap();
//     let local_clipboard = Arc::new(LocalClipboard::new());
//     let uni_clipboard = UniClipboardBuilder::new();

//     let uni_clipboard = Arc::new(uni_clipboard);
//     let uni_clipboard_clone = Arc::clone(&uni_clipboard);

//     tokio::spawn(async move {
//         uni_clipboard_clone.start().await.unwrap();
//     });

//     // 模拟睡眠状态
//     if let Some(monitor) = &uni_clipboard.key_mouse_monitor {
//         monitor.set_sleep(true).await;
//     }
//     tokio::time::sleep(std::time::Duration::from_secs(2)).await;
//     // 检查 clipboard_handler 是否暂停...

//     // 模拟唤醒状态
//     if let Some(monitor) = &uni_clipboard.key_mouse_monitor {
//         monitor.set_sleep(false).await;
//     }
//     tokio::time::sleep(std::time::Duration::from_secs(2)).await;
// }

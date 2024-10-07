// use anyhow::Result;
// use mockall::predicate::*;
// use std::sync::{Arc, Mutex};
// use std::time::Duration;
// use tokio::sync::mpsc;
// use uniclipboard::{
//     message::Payload,
//     network::WebDAVClient,
//     remote_sync::RemoteClipboardSync,
//     uni_clipboard::{UniClipboard, UniClipboardBuilder},
//     KeyMouseMonitorTrait, LocalClipboardTrait, RemoteSyncManagerTrait,
// };

// mockall::mock! {
//     LocalClipboard {}
//     #[async_trait::async_trait]
//     impl LocalClipboardTrait for LocalClipboard {
//         async fn start_monitoring(&self) -> Result<mpsc::Receiver<Payload>>;
//         async fn stop_monitoring(&self) -> Result<()>;
//         async fn pause(&self);
//         async fn resume(&self);
//         async fn set_clipboard_content(&self, content: Payload) -> Result<()>;
//         async fn read(&self) -> Result<Payload>;
//         async fn write(&self, payload: Payload) -> Result<()>;
//     }
// }

// mockall::mock! {
//     RemoteSyncManager {}
//     #[async_trait::async_trait]
//     impl RemoteSyncManagerTrait for RemoteSyncManager {
//         async fn start(&self) -> Result<()>;
//         async fn stop(&self) -> Result<()>;
//         async fn pause(&self) -> Result<()>;
//         async fn resume(&self) -> Result<()>;
//         async fn push(&self, payload: Payload) -> Result<()>;
//         async fn pull(&self, timeout: Option<Duration>) -> Result<Payload>;
//         async fn set_sync_handler(&self, handler: Arc<dyn RemoteClipboardSync>);
//         async fn sync(&self) -> Result<()>;
//     }
// }

// mockall::mock! {
//     KeyMouseMonitor {}
//     #[async_trait::async_trait]
//     impl KeyMouseMonitorTrait for KeyMouseMonitor {
//         async fn start(&self);
//         async fn is_sleep(&self) -> bool;
//     }
// }

// #[tokio::test]
// async fn test_uni_clipboard_builder_and_start() -> Result<()> {
//     let mock_clipboard = Arc::new(Mutex::new(MockLocalClipboard::new()));
//     let mock_remote_sync = Arc::new(Mutex::new(MockRemoteSyncManager::new()));
//     let mock_key_mouse_monitor = Arc::new(Mutex::new(MockKeyMouseMonitor::new()));

//     mock_clipboard.lock().unwrap()
//         .expect_start_monitoring()
//         .times(1)
//         .returning(|| Ok(mpsc::channel(100).1));

//     mock_remote_sync.lock().unwrap()
//         .expect_start()
//         .times(1)
//         .returning(|| Ok(()));

//     mock_remote_sync.lock().unwrap()
//         .expect_set_sync_handler()
//         .times(1)
//         .returning(|_| ());

//     mock_key_mouse_monitor.lock().unwrap()
//         .expect_start()
//         .times(1)
//         .returning(|| ());

//     let app = UniClipboardBuilder::new()
//         .set_local_clipboard(mock_clipboard)
//         .set_remote_sync(mock_remote_sync)
//         .set_key_mouse_monitor(mock_key_mouse_monitor)
//         .build()?;

//     app.start().await?;

//     Ok(())
// }

// #[tokio::test]
// async fn test_uni_clipboard_wait_for_stop() -> Result<()> {
//     let mock_clipboard = Arc::new(Mutex::new(MockLocalClipboard::new()));
//     let mock_remote_sync = Arc::new(Mutex::new(MockRemoteSyncManager::new()));
//     let mock_key_mouse_monitor = Arc::new(Mutex::new(MockKeyMouseMonitor::new()));

//     // 设置期望...
//     {
//         let mut clipboard = mock_clipboard.lock().unwrap();
//         clipboard.expect_start_monitoring()
//             .times(1)
//             .returning(|| Ok(mpsc::channel(100).1));
//         clipboard.expect_stop_monitoring()
//             .times(1)
//             .returning(|| Ok(()));
//     }

//     {
//         let mut remote_sync = mock_remote_sync.lock().unwrap();
//         remote_sync.expect_start()
//             .times(1)
//             .returning(|| Ok(()));
//         remote_sync.expect_stop()
//             .times(1)
//             .returning(|| Ok(()));
//         remote_sync.expect_set_sync_handler()
//             .times(1)
//             .returning(|_| ());
//     }

//     mock_key_mouse_monitor.lock().unwrap()
//         .expect_start()
//         .times(1)
//         .returning(|| ());

//     // 模拟一些操作
//     tokio::time::sleep(Duration::from_millis(100)).await;

//     // 在另一个任务中调用 stop
//     let app_clone = app.clone();
//     tokio::spawn(async move {
//         tokio::time::sleep(Duration::from_millis(50)).await;
//         app_clone.stop().await.unwrap();
//     });

//     app.wait_for_stop().await?;

//     Ok(())
// }

// // 可以添加更多测试用例来覆盖其他场景

use keyring::Entry;
use serial_test::serial;
use std::env;
use std::fs;
use std::path::PathBuf;
use tempfile::tempdir;
use uniclipboard::config::{get_config_path, Config, CONFIG};

const TEST_SERVICE_NAME: &str = "uniclipboard_test";

fn setup_test_env() -> PathBuf {
    let temp_dir = env::temp_dir().join("uniclipboard_test");
    fs::create_dir_all(&temp_dir).unwrap();
    env::set_var("UNICLIPBOARD_CONFIG_PATH", temp_dir.join("config.toml"));
    env::set_var("UNICLIPBOARD_SERVICE_NAME", TEST_SERVICE_NAME);
    temp_dir
}

fn cleanup_test_env(temp_dir: PathBuf) {
    fs::remove_dir_all(temp_dir).unwrap();
}

#[test]
#[serial]
fn test_load_config() {
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().join(".config").join("uniclipboard");
    fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");
    let config_content = r#"
        device_id = "test_device"
        webdav_url = "https://example.com/webdav"
        username = "testuser"
    "#;
    fs::write(&config_path, config_content).unwrap();

    // 重写 get_config_path 函数以返回正确的测试配置路径
    env::set_var("UNICLIPBOARD_CONFIG_PATH", config_path.to_str().unwrap());

    Config::reload().unwrap();
    let config = CONFIG.read().unwrap();
    assert_eq!(config.device_id, "test_device");
    assert_eq!(config.webdav_url, "https://example.com/webdav");
    assert_eq!(config.username, "testuser");
}

#[test]
#[serial]
#[cfg(feature = "testing")]
fn test_save_config() {
    env::set_var("UNICLIPBOARD_SERVICE_NAME", TEST_SERVICE_NAME);
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().join(".config").join("uniclipboard");
    fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");

    // 设置环境变量
    // env::set_var("HOME", temp_dir.path());
    env::set_var("UNICLIPBOARD_CONFIG_PATH", config_path.to_str().unwrap());

    let mut config = Config::default();
    config.device_id = "save_test_device".to_string();
    config.webdav_url = "https://save.example.com/webdav".to_string();
    config.username = "save_testuser".to_string();
    // config.set_password("save_testpass".to_string()).unwrap();

    config.save_without_keyring().unwrap();

    // 确保配置文件已经被创建
    assert!(config_path.exists());

    let saved_config = CONFIG.read().unwrap();
    assert_eq!(saved_config.device_id, "save_test_device");
    assert_eq!(saved_config.webdav_url, "https://save.example.com/webdav");
    assert_eq!(saved_config.username, "save_testuser");
    // assert_eq!(saved_config.get_password(), "save_testpass");

    // 清理 keyring 中的测试密码
    // if let Ok(entry) = Entry::new(TEST_SERVICE_NAME, &config.username) {
    //     entry.delete_credential().unwrap();
    // }
}

#[test]
#[serial]
fn test_get_config_path() {
    let temp_dir = tempdir().unwrap();

    // 根据不同的操作系统设置适当的环境变量
    #[cfg(target_os = "windows")]
    {
        env::set_var("USERPROFILE", temp_dir.path());
        // !无法模拟 FOLDERID_RoamingAppData，使用环境变量代替
        env::set_var(
            "UNICLIPBOARD_CONFIG_PATH",
            temp_dir.path().join("uniclipboard").join("config.toml"),
        );
    }
    #[cfg(target_os = "macos")]
    {
        env::set_var("HOME", temp_dir.path());
        // 清除可能影响测试的环境变量
        env::remove_var("UNICLIPBOARD_CONFIG_PATH");
    }
    #[cfg(target_os = "linux")]
    {
        env::set_var("XDG_CONFIG_HOME", temp_dir.path());
        // 清除可能影响测试的环境变量
        env::remove_var("UNICLIPBOARD_CONFIG_PATH");
    }

    let config_path = get_config_path().unwrap();

    // 构建预期的配置路径
    let expected_path = if cfg!(target_os = "windows") {
        temp_dir.path().join("uniclipboard").join("config.toml")
    } else if cfg!(target_os = "macos") {
        temp_dir
            .path()
            .join("Library")
            .join("Application Support")
            .join("uniclipboard")
            .join("config.toml")
    } else {
        temp_dir.path().join("uniclipboard").join("config.toml")
    };

    assert_eq!(
        config_path,
        expected_path,
        "Config path mismatch on {:?}",
        std::env::consts::OS
    );
}

#[test]
#[serial]
#[ignore = "需要手动测试"]
fn test_save_config_real() {
    // if env::var("RUN_REAL_KEYRING_TESTS").is_err() {
    //     println!("Skipping real keyring test. Set RUN_REAL_KEYRING_TESTS=1 to run.");
    //     return;
    // }

    let temp_dir = setup_test_env();
    let mut config = Config::default();
    config.device_id = "test_device".to_string();
    config.webdav_url = "https://example.com/webdav".to_string();
    config.username = "testuser".to_string();
    config.set_password("testpass".to_string()).unwrap();
    config.save().unwrap();

    let saved_config = CONFIG.read().unwrap();
    assert_eq!(saved_config.get_password(), "testpass");

    let loaded_config = Config::load().unwrap();
    assert_eq!(loaded_config.get_password(), "testpass");
    assert_eq!(loaded_config.device_id, "test_device");
    assert_eq!(loaded_config.webdav_url, "https://example.com/webdav");
    assert_eq!(loaded_config.username, "testuser");

    // 清理测试环境
    cleanup_test_env(temp_dir);

    // 清理 keyring 中的测试密码
    if let Ok(entry) = Entry::new(TEST_SERVICE_NAME, &config.username) {
        entry.delete_credential().unwrap();
    }
}

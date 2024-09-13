use uniclipboard::config::{Config, get_config_path, CONFIG};
use std::fs;
use tempfile::tempdir;
use std::env;
use serial_test::serial;

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
        password = "testpass"
    "#;
    fs::write(&config_path, config_content).unwrap();

    env::set_var("HOME", temp_dir.path());

    // 重写 get_config_path 函数以返回正确的测试配置路径
    env::set_var("UNICLIPBOARD_CONFIG_PATH", config_path.to_str().unwrap());

    Config::load().unwrap();
    let config = CONFIG.read().unwrap();
    assert_eq!(config.device_id, "test_device");
    assert_eq!(config.webdav_url, "https://example.com/webdav");
    assert_eq!(config.username, "testuser");
    assert_eq!(config.password, "testpass");
}

#[test]
#[serial]
fn test_save_config() {
    let temp_dir = tempdir().unwrap();
    let config_dir = temp_dir.path().join(".config").join("uniclipboard");
    fs::create_dir_all(&config_dir).unwrap();
    let config_path = config_dir.join("config.toml");

    // 设置环境变量
    env::set_var("HOME", temp_dir.path());
    env::set_var("UNICLIPBOARD_CONFIG_PATH", config_path.to_str().unwrap());

    let config = Config {
        device_id: "save_test_device".to_string(),
        webdav_url: "https://save.example.com/webdav".to_string(),
        username: "save_testuser".to_string(),
        password: "save_testpass".to_string(),
        push_interval: Some(500),
        pull_interval: Some(500),
        sync_interval: Some(500),
        enable_push: Some(true),
        enable_pull: Some(true),
    };

    config.save().unwrap();

    // 确保配置文件已经被创建
    assert!(config_path.exists());

    let saved_config = CONFIG.read().unwrap();
    assert_eq!(saved_config.device_id, "save_test_device");
    assert_eq!(saved_config.webdav_url, "https://save.example.com/webdav");
    assert_eq!(saved_config.username, "save_testuser");
    assert_eq!(saved_config.password, "save_testpass");
}

#[test]
fn test_get_config_path() {
    let temp_dir = tempdir().unwrap();
    
    // 根据不同的操作系统设置适当的环境变量
    #[cfg(target_os = "windows")]
    {
        env::set_var("USERPROFILE", temp_dir.path());
    }
    #[cfg(target_os = "macos")]
    {
        env::set_var("HOME", temp_dir.path());
    }
    #[cfg(target_os = "linux")]
    {
        env::set_var("HOME", temp_dir.path());
    }

    // 清除可能影响测试的环境变量
    env::remove_var("UNICLIPBOARD_CONFIG_PATH");

    let config_path = get_config_path().unwrap();
    
    // 构建预期的配置路径
    let expected_path = if cfg!(target_os = "windows") {
        temp_dir.path().join("AppData").join("Roaming").join("uniclipboard").join("config.toml")
    } else if cfg!(target_os = "macos") {
        temp_dir.path().join("Library").join("Application Support").join("uniclipboard").join("config.toml")
    } else {
        temp_dir.path().join(".config").join("uniclipboard").join("config.toml")
    };

    assert_eq!(config_path, expected_path, "Config path mismatch on {:?}", std::env::consts::OS);
}
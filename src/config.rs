use anyhow::{Context, Result};
use keyring::Entry;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;
use uuid::Uuid;

pub static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| {
    let config = Config::load().unwrap();
    RwLock::new(config)
});

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub device_id: String,
    pub webdav_url: String,
    pub username: String,
    #[serde(skip)]
    password: String,
    pub push_interval: Option<u64>, // ms
    pub pull_interval: Option<u64>, // ms
    pub sync_interval: Option<u64>, // ms
    pub enable_push: Option<bool>,
    pub enable_pull: Option<bool>,
    pub enable_key_mouse_monitor: Option<bool>,
    pub key_mouse_monitor_sleep_timeout: Option<u64>, // ms
}

fn generate_device_id() -> String {
    // 随机生成 6 位字母
    let device_id = Uuid::new_v4().to_string();
    device_id.chars().take(6).collect()
}

impl Config {
    pub fn default() -> Self {
        Self {
            device_id: generate_device_id(),
            webdav_url: String::new(),
            username: String::new(),
            password: String::new(),
            push_interval: Some(500),
            pull_interval: Some(500),
            sync_interval: Some(500),
            enable_push: Some(true),
            enable_pull: Some(true),
            enable_key_mouse_monitor: Some(true),
            key_mouse_monitor_sleep_timeout: Some(5000),
        }
    }

    pub fn merge_with_default(&mut self) {
        let default_config = Config::default();
        if self.enable_key_mouse_monitor.is_none() {
            self.enable_key_mouse_monitor = default_config.enable_key_mouse_monitor;
        }
        if self.key_mouse_monitor_sleep_timeout.is_none() {
            self.key_mouse_monitor_sleep_timeout = default_config.key_mouse_monitor_sleep_timeout;
        }
    }

    pub fn load() -> Result<Self> {
        let config_path = get_config_path()?;
        if let Some(config_str) = fs::read_to_string(&config_path).ok() {
            let mut config: Config =
                toml::from_str(&config_str).with_context(|| "无法解析配置文件")?;
            config.merge_with_default();
            // 如果配置文件中有密码，使用它并迁移到 keyring
            if !config.password.is_empty() {
                let entry = Entry::new(&Self::get_service_name(), &config.username)?;
                entry.set_password(&config.password)?;

                // 从配置文件中删除密码
                config.password = String::new();
                config.save()?;
            } else {
                let password =
                    if let Ok(entry) = Entry::new(&Self::get_service_name(), &config.username) {
                        entry.get_password()?
                    } else {
                        String::new()
                    };
                config.password = password.clone();
            }

            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    #[allow(dead_code)]
    pub fn reload() -> Result<()> {
        let config = Config::load()?;
        CONFIG.write().unwrap().clone_from(&config);
        Ok(())
    }

    pub fn save(&self) -> Result<()> {
        let config_path = get_config_path()?;
        let config_str = toml::to_string(self)?;
        fs::create_dir_all(config_path.parent().unwrap())?;
        fs::write(&config_path, config_str)
            .with_context(|| format!("无法写入配置文件: {:?}", config_path))?;

        // 将密码保存到 keyring
        let entry = Entry::new(&Self::get_service_name(), &self.username)?;
        entry.set_password(&self.password)?;

        CONFIG.write().unwrap().clone_from(self);
        Ok(())
    }

    pub fn save_without_keyring(&self) -> Result<()> {
        let config_path = get_config_path()?;
        let config_str = toml::to_string(self)?;
        fs::create_dir_all(config_path.parent().unwrap())?;
        fs::write(&config_path, config_str)
            .with_context(|| format!("无法写入配置文件: {:?}", config_path))?;

        CONFIG.write().unwrap().clone_from(self);
        Ok(())
    }

    fn get_service_name() -> String {
        env::var("UNICLIPBOARD_SERVICE_NAME").unwrap_or_else(|_| "uniclipboard".to_string())
    }

    pub fn get_device_id(&self) -> String {
        self.device_id.clone()
    }

    pub fn get_webdav_url(&self) -> String {
        self.webdav_url.clone()
    }

    pub fn get_username(&self) -> String {
        self.username.clone()
    }

    pub fn get_password(&self) -> String {
        if self.password.is_empty() {
            if let Ok(entry) = Entry::new(&Self::get_service_name(), &self.username) {
                entry.get_password().unwrap_or_default()
            } else {
                String::new()
            }
        } else {
            self.password.clone()
        }
    }

    /// 设置密码，不保存到 keyring
    ///
    /// 如果需要将密码保存到 keyring，请使用 `save` 方法
    pub fn set_password(&mut self, password: String) -> Result<()> {
        self.password = password;
        Ok(())
    }
}

pub fn get_config_path() -> Result<PathBuf> {
    if let Ok(path) = env::var("UNICLIPBOARD_CONFIG_PATH") {
        return Ok(PathBuf::from(path));
    }

    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join("uniclipboard");
    Ok(config_dir.join("config.toml"))
}

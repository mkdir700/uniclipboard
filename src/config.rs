use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use anyhow::{Result, Context};
use std::env;
use once_cell::sync::Lazy;
use std::sync::RwLock;

pub static CONFIG: Lazy<RwLock<Config>> = Lazy::new(|| {
    RwLock::new(Config::default())
});

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct Config {
    pub device_id: String,
    pub webdav_url: String,
    pub username: String,
    pub password: String,
    pub push_interval: Option<u64>,  // ms
    pub pull_interval: Option<u64>,  // ms
    pub sync_interval: Option<u64>,  // ms
    pub enable_push: Option<bool>,
    pub enable_pull: Option<bool>,
}

impl Config {
    pub fn default() -> Self {
        Self {
            device_id: String::new(),
            webdav_url: String::new(),
            username: String::new(),
            password: String::new(),
            push_interval: Some(500),
            pull_interval: Some(500),
            sync_interval: Some(500),
            enable_push: Some(true),
            enable_pull: Some(true),
        }
    }

    pub fn load() -> Result<Self> {
        let config_path = get_config_path()?;
        let config_str = fs::read_to_string(&config_path)
            .with_context(|| format!("Could not read config file: {:?}", config_path))?;
        let config: Config = toml::from_str(&config_str)
            .with_context(|| "Could not parse config file")?;
        *CONFIG.write().unwrap() = config.clone();
        Ok(config)
    }

    pub fn save(&self) -> Result<()> {
        let config_path = get_config_path()?;
        let config_str = toml::to_string(self)?;
        fs::create_dir_all(config_path.parent().unwrap())?;
        fs::write(&config_path, config_str)
            .with_context(|| format!("Could not write config file: {:?}", config_path))?;
        CONFIG.write().unwrap().clone_from(self);
        Ok(())
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
        self.password.clone()
    }
}

pub fn get_config_path() -> anyhow::Result<PathBuf> {
    if let Ok(path) = env::var("UNICLIPBOARD_CONFIG_PATH") {
        return Ok(PathBuf::from(path));
    }

    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not find config directory"))?
        .join("uniclipboard");
    Ok(config_dir.join("config.toml"))
}

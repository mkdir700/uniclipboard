use log::warn;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub ip: Option<String>,
    pub port: Option<u16>,
}

pub struct DeviceManager {
    devices: HashMap<String, Device>,
}

pub static GLOBAL_DEVICE_MANAGER: Lazy<Mutex<DeviceManager>> =
    Lazy::new(|| Mutex::new(DeviceManager::new()));

// 可选：添加一个便捷函数来获取 DeviceManager 的引用
pub fn get_device_manager() -> &'static Mutex<DeviceManager> {
    &GLOBAL_DEVICE_MANAGER
}

impl Device {
    pub fn new(id: String, ip: Option<String>, port: Option<u16>) -> Self {
        Self { id, ip, port }
    }
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
        }
    }

    pub fn add(&mut self, device: Device) {
        let id = device.id.clone();
        if self.devices.contains_key(&id) {
            warn!("Device will be overwrited: {}", id);
        }
        self.devices.insert(id, device);
    }

    //pub fn remove(&mut self, device_id: &str) -> Option<Device> {
    //    self.devices.remove(device_id)
    //}
    //
    //pub fn get(&self, device_id: &str) -> Option<&Device> {
    //    self.devices.get(device_id)
    //}
    //
    //pub fn has(&self, device_id: &str) -> bool {
    //    self.devices.contains_key(device_id)
    //}

    pub fn get_by_device_id(&self, device_id: &str) -> Option<&Device> {
        self.devices
            .iter()
            .find(|(_, device)| device.id == device_id)
            .map(|(_, device)| device)
    }
}

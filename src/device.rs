use log::warn;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{self, Display};
use std::sync::Mutex;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub ip: Option<String>,
    pub port: Option<u16>,
    pub server_port: Option<u16>,
}

pub struct DeviceManager {
    devices: HashMap<String, Device>,
    self_device: Option<String>,
}

pub static GLOBAL_DEVICE_MANAGER: Lazy<Mutex<DeviceManager>> =
    Lazy::new(|| Mutex::new(DeviceManager::new()));

pub static NEW_DEVICE_BROADCASTER: Lazy<broadcast::Sender<Device>> = Lazy::new(|| {
    let (sender, _) = broadcast::channel(20);
    sender
});

// 可选：添加一个便捷函数来获取 DeviceManager 的引用
pub fn get_device_manager() -> &'static Mutex<DeviceManager> {
    &GLOBAL_DEVICE_MANAGER
}

// 新增：全局函数用于订阅新设备
pub fn subscribe_new_devices() -> broadcast::Receiver<Device> {
    NEW_DEVICE_BROADCASTER.subscribe()
}

impl Device {
    pub fn new(
        id: String,
        ip: Option<String>,
        port: Option<u16>,
        server_port: Option<u16>,
    ) -> Self {
        Self {
            id,
            ip,
            port,
            server_port,
        }
    }
}

impl PartialEq for Device {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Device(id: {}, ip: {}, port: {}, server_port: {})",
            self.id,
            self.ip.clone().unwrap_or_default(),
            self.port.clone().unwrap_or_default(),
            self.server_port.clone().unwrap_or_default()
        )
    }
}

impl DeviceManager {
    pub fn new() -> Self {
        Self {
            devices: HashMap::new(),
            self_device: None,
        }
    }

    pub fn set_self_device(&mut self, device: &Device) {
        self.self_device = Some(device.id.clone());
    }

    pub fn add(&mut self, device: Device) {
        let id = device.id.clone();
        if self.devices.contains_key(&id) {
            warn!("Device will be overwritten: {}", id);
        }
        self.devices.insert(id, device.clone());
        let _ = NEW_DEVICE_BROADCASTER.send(device);
    }

    // pub fn merge(&mut self, devices: &Vec<Device>) {
    //     // ? 是否需要增加一个时间戳字段，可用于在合并时进行比对
    //     for device in devices {
    //         self.add(device.clone());
    //     }
    // }

    /// 合并并返回新增的 Device
    pub fn merge_and_get_new(&mut self, devices: &Vec<Device>) -> Vec<Device> {
        let mut new_devices = Vec::new();
        for device in devices {
            if !self.has(&device.id) {
                new_devices.push(device.clone());
            }
        }
        for device in &new_devices {
            self.add(device.clone());
        }
        new_devices
    }

    pub fn remove(&mut self, device_id: &str) -> Option<Device> {
        self.devices.remove(device_id)
    }

    #[allow(dead_code)]
    pub fn get(&self, device_id: &str) -> Option<&Device> {
        self.devices.get(device_id)
    }

    #[allow(dead_code)]
    pub fn has(&self, device_id: &str) -> bool {
        self.devices.contains_key(device_id)
    }

    pub fn get_all_devices(&self) -> Vec<&Device> {
        self.devices.values().collect()
    }

    // 获取除了自己的所有设备
    pub fn get_all_devices_except_self(&self) -> Vec<&Device> {
        self.devices
            .values()
            .filter(|device| {
                if let Some(self_device) = &self.self_device {
                    device.id != *self_device
                } else {
                    true
                }
            })
            .collect()
    }

    pub fn clear(&mut self) {
        self.devices.clear();
    }

    /// 通过 ip 和 port 获取设备
    pub fn get_device_by_ip_and_port(&self, ip: &str, port: u16) -> Option<&Device> {
        self.devices.values().find(|device| {
            device.ip.is_some()
                && device.ip.as_ref().unwrap() == ip
                && device.port.is_some()
                && device.port.unwrap() == port
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_manager() {
        let mut manager = DeviceManager::new();
        let device = Device::new("test".to_string(), None, None, None);
        manager.add(device.clone());
        assert_eq!(manager.get("test"), Some(&device));
        assert_eq!(manager.get("test1"), None);
    }

    #[test]
    fn test_device_eq() {
        let device1 = Device::new("test".to_string(), None, None, None);
        let device2 = Device::new("test".to_string(), None, None, None);
        assert_eq!(device1, device2);
    }

    #[test]
    fn test_device_manager_has() {
        let mut manager = DeviceManager::new();
        let device = Device::new("test".to_string(), None, None, None);
        manager.add(device.clone());
        assert_eq!(manager.has("test"), true);
        assert_eq!(manager.has("test1"), false);
    }
}

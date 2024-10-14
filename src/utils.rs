use sha2::{Sha256, Digest};

pub fn string_to_32_bytes(input: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().into()
}

pub fn generate_device_id() -> String {
    // 生成6位随机数字
    let random_number = rand::random::<u32>() % 1000000;
    format!("{:06}", random_number)
}

/// 检查 IP 地址是否有效
pub fn is_valid_ip(ip: &str) -> bool {
    let parts: Vec<&str> = ip.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    for part in parts {
        if part.parse::<u8>().is_err() {
            return false;
        }
    }
    true
}

/// 检查端口是否有效
pub fn is_valid_port(port: u16) -> bool {
    port >= 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_to_32_bytes() {
        let input = "test";
        let output = string_to_32_bytes(input);
        assert_eq!(output.len(), 32);
    }

    #[test]
    fn test_generate_device_id() {
        let id = generate_device_id();
        assert_eq!(id.len(), 6);
    }

    #[test]
    fn test_is_valid_ip() {
        assert!(is_valid_ip("192.168.1.1"));
        assert!(is_valid_ip("0.0.0.0"));
        assert!(is_valid_ip("255.255.255.255"));
        assert!(!is_valid_ip("256.256.256.256"));
        assert!(!is_valid_ip("192.168.1"));
        assert!(!is_valid_ip("192.168.1.1.1"));
        assert!(!is_valid_ip("192.168.1.a"));
        assert!(!is_valid_ip("..."));
    }

    #[test]
    fn test_is_valid_port() {
        assert!(!is_valid_port(0));
        assert!(!is_valid_port(1023));
        assert!(is_valid_port(1024));
        assert!(is_valid_port(8080));
        assert!(is_valid_port(65535));
    }
}

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
        if let Ok(num) = part.parse::<u8>() {
            if num > 255 {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

/// 检查端口是否有效
pub fn is_valid_port(port: u16) -> bool {
    port >= 1024 && port <= 65535
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
        assert_eq!(is_valid_ip("192.168.1.1"), true);
        assert_eq!(is_valid_ip("256.256.256.256"), false);
    }
}

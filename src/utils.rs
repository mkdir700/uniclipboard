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
}

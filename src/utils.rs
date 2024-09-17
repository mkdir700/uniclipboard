use sha2::{Sha256, Digest};

pub fn string_to_32_bytes(input: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hasher.finalize().into()
}

// // 使用示例
// let key_str = "我的密钥";
// let key_bytes: [u8; 32] = string_to_32_bytes(key_str);
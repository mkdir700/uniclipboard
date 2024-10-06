use base64::Engine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json;
use std::fmt;
use twox_hash::xxh3::hash64;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Payload {
    Text(TextPayload),
    Image(ImagePayload),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextPayload {
    #[serde(
        serialize_with = "serialize_bytes",
        deserialize_with = "deserialize_bytes"
    )]
    pub content: Bytes,
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
}

impl TextPayload {
    #[allow(dead_code)]
    pub fn text(&self) -> &str {
        std::str::from_utf8(self.content.as_ref()).unwrap()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ImagePayload {
    #[serde(
        serialize_with = "serialize_bytes",
        deserialize_with = "deserialize_bytes"
    )]
    pub content: Bytes,
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
    pub width: usize,
    pub height: usize,
    pub format: String,
    pub size: usize,
}

impl ImagePayload {
    // 新增方法：计算图片内容的哈希值
    pub fn content_hash(&self) -> u64 {
        hash64(&self.content)
    }

    pub fn is_similar(&self, other: &ImagePayload) -> bool {
        // 尺寸一致且文件大小相差不超过 3%
        self.width == other.width
            && self.height == other.height
            && (self.size as f64 - other.size as f64).abs() / (self.size as f64) <= 0.03
    }
}

fn serialize_bytes<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let base64_string = base64::engine::general_purpose::STANDARD.encode(bytes);
    serializer.serialize_str(&base64_string)
}

fn deserialize_bytes<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let base64_string = String::deserialize(deserializer)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&base64_string)
        .map_err(|e| serde::de::Error::custom(e.to_string()))?;
    Ok(Bytes::from(bytes))
}

impl Payload {
    pub fn new_text(content: Bytes, device_id: String, timestamp: DateTime<Utc>) -> Self {
        Payload::Text(TextPayload {
            content,
            device_id,
            timestamp,
        })
    }

    pub fn new_image(
        content: Bytes,
        device_id: String,
        timestamp: DateTime<Utc>,
        width: usize,
        height: usize,
        format: String,
        size: usize,
    ) -> Self {
        Payload::Image(ImagePayload {
            content,
            device_id,
            timestamp,
            width,
            height,
            format,
            size,
        })
    }

    #[allow(dead_code)]
    pub fn get_content(&self) -> &Bytes {
        match self {
            Payload::Text(p) => &p.content,
            Payload::Image(p) => &p.content,
        }
    }

    #[allow(dead_code)]
    pub fn get_timestamp(&self) -> DateTime<Utc> {
        match self {
            Payload::Text(p) => p.timestamp,
            Payload::Image(p) => p.timestamp,
        }
    }

    #[allow(dead_code)]
    pub fn is_image(&self) -> bool {
        matches!(self, Payload::Image(_))
    }

    #[allow(dead_code)]
    pub fn as_image(&self) -> Option<&ImagePayload> {
        if let Payload::Image(image) = self {
            Some(image)
        } else {
            None
        }
    }

    pub fn get_device_id(&self) -> &str {
        match self {
            Payload::Text(p) => &p.device_id,
            Payload::Image(p) => &p.device_id,
        }
    }

    /// 获取 Payload 的唯一标识符
    pub fn get_key(&self) -> String {
        match self {
            Payload::Text(p) => {
                format!("{:016x}", hash64(p.content.as_ref()))
            }
            Payload::Image(p) => {
                // 使用图片内容哈希 + 尺寸信息作为唯一标识符
                let content_hash = p.content_hash();
                let size_info = format!("{}x{}", p.width, p.height);
                format!("img_{:016x}_{}", content_hash, size_info)
            }
        }
    }

    /// 判断两个 Payload 是否相同
    ///
    /// 文本消息只比较内容是否相同
    /// 图片消息只比较内容是否相似，不要求完全相同
    pub fn is_duplicate(&self, other: &Payload) -> bool {
        match (self, other) {
            (Payload::Text(t1), Payload::Text(t2)) => t1.content == t2.content,
            (Payload::Image(i1), Payload::Image(i2)) => i1.is_similar(i2),
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn eq(&self, other: &Payload) -> bool {
        self.get_key() == other.get_key()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

// 友好的展示大小
fn friendly_size(size: usize) -> String {
    if size < 1024 {
        format!("{} B", size)
    } else if size < 1024 * 1024 {
        format!("{} KB", size / 1024)
    } else {
        format!("{} MB", size / 1024 / 1024)
    }
}

impl fmt::Display for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Payload::Text(text) => write!(
                f,
                "文本消息 - KEY: {}, 设备: {}, 时间: {}, 内容长度: {}",
                self.get_key(),
                text.device_id,
                text.timestamp,
                friendly_size(text.content.len())
            ),
            Payload::Image(image) => write!(
                f,
                "图片消息 - KEY: {}, 设备: {}, 时间: {}, 尺寸: {}x{}, 格式: {}, 大小: {}",
                self.get_key(),
                image.device_id,
                image.timestamp,
                image.width,
                image.height,
                image.format,
                friendly_size(image.size)
            ),
        }
    }
}

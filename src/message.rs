use base64::Engine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use hex;
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::{Digest, Sha256};
use std::fmt;

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

    pub fn get_content(&self) -> &Bytes {
        match self {
            Payload::Text(p) => &p.content,
            Payload::Image(p) => &p.content,
        }
    }

    pub fn get_device_id(&self) -> &str {
        match self {
            Payload::Text(p) => &p.device_id,
            Payload::Image(p) => &p.device_id,
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

    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.get_content());
        hex::encode(hasher.finalize())
    }

    pub fn eq(&self, other: &Payload) -> bool {
        // TODO: 使用更高效的方式比较两个 Payload 是否相等
        //  比如对于图片类型，比较图片的大小、格式、尺寸等
        self.hash() == other.hash()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

impl fmt::Display for Payload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Payload::Text(text) => write!(
                f,
                "文本消息 - 设备: {}, 时间: {}, 内容长度: {}",
                text.device_id,
                text.timestamp,
                text.content.len()
            ),
            Payload::Image(image) => write!(
                f,
                "图片消息 - 设备: {}, 时间: {}, 尺寸: {}x{}, 格式: {}, 大小: {}",
                image.device_id,
                image.timestamp,
                image.width,
                image.height,
                image.format,
                image.size
            ),
        }
    }
}

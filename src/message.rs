use base64::Engine;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use hex;
use serde::{Deserialize, Serialize};
use serde_json;
use sha2::{Digest, Sha256};


#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Payload {
    #[serde(
        serialize_with = "serialize_bytes",
        deserialize_with = "deserialize_bytes"
    )]
    pub content: Bytes,
    pub content_type: String,
    pub device_id: String,
    pub timestamp: DateTime<Utc>,
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
    pub fn new(
        content: Bytes,
        content_type: String,
        device_id: String,
        timestamp: DateTime<Utc>,
    ) -> Self {
        Payload {
            content,
            content_type,
            device_id,
            timestamp,
        }
    }

    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(&self.content);
        hex::encode(hasher.finalize())
    }

    pub fn eq(&self, other: &Payload) -> bool {
        self.hash() == other.hash()
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
}

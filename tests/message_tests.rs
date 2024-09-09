use uniclipboard::Payload;
use bytes::Bytes;
use chrono::Utc;
use serde_json;

#[test]
fn test_payload_new() {
    let content = Bytes::from("测试内容");
    let content_type = "text".to_string();
    let device_id = "test_device".to_string();
    let timestamp = Utc::now();

    let payload = Payload::new(content.clone(), content_type.clone(), device_id.clone(), timestamp);

    assert_eq!(payload.content, content);
    assert_eq!(payload.content_type, content_type);
    assert_eq!(payload.device_id, device_id);
    assert_eq!(payload.timestamp, timestamp);
}

#[test]
fn test_payload_hash() {
    let payload1 = Payload::new(
        Bytes::from("内容1"),
        "text".to_string(),
        "device1".to_string(),
        Utc::now(),
    );
    let payload2 = Payload::new(
        Bytes::from("内容1"),
        "text".to_string(),
        "device2".to_string(),
        Utc::now(),
    );
    let payload3 = Payload::new(
        Bytes::from("内容2"),
        "text".to_string(),
        "device1".to_string(),
        Utc::now(),
    );

    assert_eq!(payload1.hash(), payload2.hash());
    assert_ne!(payload1.hash(), payload3.hash());
}

#[test]
fn test_payload_eq() {
    let payload1 = Payload::new(
        Bytes::from("内容"),
        "text".to_string(),
        "device1".to_string(),
        Utc::now(),
    );
    let payload2 = Payload::new(
        Bytes::from("内容"),
        "text".to_string(),
        "device2".to_string(),
        Utc::now(),
    );
    let payload3 = Payload::new(
        Bytes::from("不同内容"),
        "text".to_string(),
        "device1".to_string(),
        Utc::now(),
    );

    assert!(payload1.eq(&payload2));
    assert!(!payload1.eq(&payload3));
}

#[test]
fn test_payload_to_json() {
    let content = Bytes::from("JSON测试");
    let content_type = "text".to_string();
    let device_id = "json_device".to_string();
    let timestamp = Utc::now();

    let payload = Payload::new(content.clone(), content_type.clone(), device_id.clone(), timestamp);

    let json_string = payload.to_json();
    let deserialized: Payload = serde_json::from_str(&json_string).unwrap();

    assert_eq!(deserialized.content, content);
    assert_eq!(deserialized.content_type, content_type);
    assert_eq!(deserialized.device_id, device_id);
    assert_eq!(deserialized.timestamp, timestamp);
}

#[test]
fn test_payload_serialization() {
    let payload = Payload::new(
        Bytes::from("序列化测试"),
        "text".to_string(),
        "serialize_device".to_string(),
        Utc::now(),
    );

    let serialized = serde_json::to_string(&payload).unwrap();
    let deserialized: Payload = serde_json::from_str(&serialized).unwrap();

    assert_eq!(payload.content, deserialized.content);
    assert_eq!(payload.content_type, deserialized.content_type);
    assert_eq!(payload.device_id, deserialized.device_id);
    assert_eq!(payload.timestamp, deserialized.timestamp);
}

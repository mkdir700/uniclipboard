use uniclipboard::Args;
use clap::Parser;

#[test]
fn test_parse_args_with_all_options() {
    let args = Args::parse_from([
        "test",
        "--webdav-url", "https://example.com/webdav",
        "--username", "testuser",
        "--password", "testpass"
    ]);

    assert_eq!(args.webdav_url, Some("https://example.com/webdav".to_string()));
    assert_eq!(args.username, Some("testuser".to_string()));
    assert_eq!(args.password, Some("testpass".to_string()));
}

#[test]
fn test_parse_args_with_short_options() {
    let args = Args::parse_from([
        "test",
        "-w", "https://example.com/webdav",
        "-u", "testuser",
        "-p", "testpass"
    ]);

    assert_eq!(args.webdav_url, Some("https://example.com/webdav".to_string()));
    assert_eq!(args.username, Some("testuser".to_string()));
    assert_eq!(args.password, Some("testpass".to_string()));
}

#[test]
fn test_parse_args_function() {
    let args = parse_args_from(vec![
        "test".to_string(),
        "--webdav-url".to_string(), "https://example.com/webdav".to_string(),
        "--username".to_string(), "testuser".to_string(),
        "--password".to_string(), "testpass".to_string()
    ]);

    assert_eq!(args.webdav_url, Some("https://example.com/webdav".to_string()));
    assert_eq!(args.username, Some("testuser".to_string()));
    assert_eq!(args.password, Some("testpass".to_string()));
}

// 添加这个辅助函数用于测试
fn parse_args_from(args: Vec<String>) -> Args {
    Args::parse_from(args)
}

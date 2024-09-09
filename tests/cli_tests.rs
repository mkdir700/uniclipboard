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

    assert_eq!(args.webdav_url, "https://example.com/webdav");
    assert_eq!(args.username, "testuser");
    assert_eq!(args.password, "testpass");
}

#[test]
fn test_parse_args_with_short_options() {
    let args = Args::parse_from([
        "test",
        "-w", "https://example.com/webdav",
        "-u", "testuser",
        "-p", "testpass"
    ]);

    assert_eq!(args.webdav_url, "https://example.com/webdav");
    assert_eq!(args.username, "testuser");
    assert_eq!(args.password, "testpass");
}

#[test]
fn test_parse_args_missing_required_option() {
    let result = Args::try_parse_from([
        "test",
        "--username", "testuser",
        "--password", "testpass"
    ]);

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("--webdav-url"));
}

#[test]
fn test_parse_args_function() {
    let args = parse_args_from(vec![
        "test".to_string(),
        "--webdav-url".to_string(), "https://example.com/webdav".to_string(),
        "--username".to_string(), "testuser".to_string(),
        "--password".to_string(), "testpass".to_string()
    ]);

    assert_eq!(args.webdav_url, "https://example.com/webdav");
    assert_eq!(args.username, "testuser");
    assert_eq!(args.password, "testpass");
}

// 添加这个辅助函数用于测试
fn parse_args_from(args: Vec<String>) -> Args {
    Args::parse_from(args)
}

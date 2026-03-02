use rpg_encoder::RpgError;
use std::io;
use std::path::PathBuf;

#[test]
fn test_error_io_from() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
    let rpg_err: RpgError = io_err.into();

    assert!(matches!(rpg_err, RpgError::Io(_)));
    assert!(format!("{}", rpg_err).contains("IO error"));
}

#[test]
fn test_error_json_from() {
    let json_err = serde_json::from_str::<i32>("not a number").unwrap_err();
    let rpg_err: RpgError = json_err.into();

    assert!(matches!(rpg_err, RpgError::JsonError(_)));
    assert!(format!("{}", rpg_err).contains("JSON"));
}

#[test]
fn test_error_display_parse_error() {
    let err = RpgError::parse_error(PathBuf::from("test.rs"), 10, 5, "unexpected token");

    let msg = format!("{}", err);
    assert!(msg.contains("test.rs"));
    assert!(msg.contains("unexpected token"));
    assert!(msg.contains("10:5"));
}

#[test]
fn test_error_display_tree_sitter() {
    let err = RpgError::tree_sitter_error(PathBuf::from("test.rs"), "parse failed");
    let msg = format!("{}", err);
    assert!(msg.contains("Tree-sitter"));
    assert!(msg.contains("test.rs"));
}

#[test]
fn test_error_display_language_not_supported() {
    let err = RpgError::LanguageNotSupported("brainfuck".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("brainfuck"));
    assert!(msg.contains("not supported"));
}

#[test]
fn test_error_display_no_parser() {
    let err = RpgError::NoParser("unknown.xyz".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("No parser"));
    assert!(msg.contains("unknown.xyz"));
}

#[test]
fn test_error_display_invalid_path() {
    let err = RpgError::InvalidPath("/bad/path".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("Invalid path"));
}

#[test]
fn test_error_display_node_not_found() {
    let err = RpgError::NodeNotFound("node_123".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("Node not found"));
    assert!(msg.contains("node_123"));
}

#[test]
fn test_error_display_config() {
    let err = RpgError::Config("custom error".to_string());
    let msg = format!("{}", err);
    assert!(msg.contains("custom error"));
}

#[test]
fn test_error_debug() {
    let err = RpgError::InvalidPath("test".to_string());
    let debug_msg = format!("{:?}", err);
    assert!(debug_msg.contains("InvalidPath"));
}

#[test]
fn test_error_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<RpgError>();
}

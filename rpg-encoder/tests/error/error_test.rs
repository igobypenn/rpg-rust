use rpg_encoder::{RpgEncoder, RpgError};

#[test]
fn test_encode_nonexistent_path() {
    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(std::path::Path::new("/nonexistent/path/12345"));

    assert!(result.is_err());
    if let Err(RpgError::InvalidPath(msg)) = result {
        assert!(msg.contains("does not exist"));
    } else {
        panic!("Expected InvalidPath error");
    }
}

#[test]
fn test_encode_file_not_directory() {
    let temp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp.path(), "fn main() {}").unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(temp.path());

    assert!(result.is_err());
    if let Err(RpgError::InvalidPath(msg)) = result {
        assert!(msg.contains("not a directory"));
    } else {
        panic!("Expected InvalidPath error");
    }
}

#[test]
fn test_to_json_before_encode() {
    let encoder = RpgEncoder::new().unwrap();
    let result = encoder.to_json();

    assert!(result.is_err());
}

#[test]
fn test_empty_directory() {
    let dir = tempfile::tempdir().unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(dir.path());

    assert!(result.is_ok());
    let encode_result = result.unwrap();
    assert_eq!(encode_result.graph.node_count(), 1);
}

#[test]
fn test_unsupported_extension() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("data.txt");
    std::fs::write(&file, "some text data").unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(dir.path());

    assert!(result.is_ok());
    let encode_result = result.unwrap();
    assert_eq!(encode_result.graph.node_count(), 1);
}

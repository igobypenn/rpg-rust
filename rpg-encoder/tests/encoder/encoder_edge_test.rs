use rpg_encoder::RpgEncoder;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

fn create_test_file(dir: &std::path::Path, name: &str, content: &str) -> std::io::Result<PathBuf> {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut f = std::fs::File::create(&path)?;
    write!(f, "{}", content)?;
    Ok(path)
}

#[test]
fn test_encode_symlink_directory() {
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("target");
    std::fs::create_dir(&target).unwrap();
    create_test_file(&target, "main.rs", "fn main() {}").unwrap();

    let link = dir.path().join("link");
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let mut encoder = RpgEncoder::new().unwrap();
        let result = encoder.encode(&link);
        assert!(result.is_ok());
    }
}

#[test]
fn test_encode_binary_file_skipped() {
    let dir = TempDir::new().unwrap();

    let binary_path = dir.path().join("data.bin");
    let mut f = std::fs::File::create(&binary_path).unwrap();
    f.write_all(&[0x00, 0xFF, 0xFE, 0x01, 0x02]).unwrap();

    create_test_file(dir.path(), "main.rs", "fn main() {}").unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(dir.path());

    assert!(result.is_ok());
    let encode_result = result.unwrap();
    let file_nodes: Vec<_> = encode_result
        .graph
        .nodes()
        .filter(|n| n.category == rpg_encoder::NodeCategory::File)
        .collect();

    assert!(file_nodes
        .iter()
        .all(|n| !n.path.as_ref().unwrap().ends_with("data.bin")));
}

#[test]
fn test_encode_very_large_file() {
    let dir = TempDir::new().unwrap();

    let large_content = "fn dummy() {}\n".repeat(10000);
    create_test_file(dir.path(), "large.rs", &large_content).unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(dir.path());

    assert!(result.is_ok());
}

#[test]
fn test_encode_deeply_nested_directory() {
    let dir = TempDir::new().unwrap();

    let mut nested = dir.path().to_path_buf();
    for i in 0..20 {
        nested = nested.join(format!("level{}", i));
    }
    create_test_file(&nested, "main.rs", "fn main() {}").unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(dir.path());

    assert!(result.is_ok());
}

#[test]
fn test_encode_filename_special_chars() {
    let dir = TempDir::new().unwrap();

    create_test_file(dir.path(), "test file.rs", "fn main() {}").unwrap();
    create_test_file(dir.path(), "test-file.rs", "fn main() {}").unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(dir.path());

    assert!(result.is_ok());
}

#[test]
fn test_encode_no_source_files() {
    let dir = TempDir::new().unwrap();

    create_test_file(dir.path(), "README.md", "# Readme").unwrap();
    create_test_file(dir.path(), "data.txt", "some data").unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(dir.path());

    assert!(result.is_ok());
    let encode_result = result.unwrap();
    assert_eq!(encode_result.graph.node_count(), 1);
}

#[test]
fn test_encode_to_json_before_encode_error() {
    let encoder = RpgEncoder::new().unwrap();
    let result = encoder.to_json();

    assert!(result.is_err());
}

#[test]
fn test_encode_to_json_compact() {
    let dir = TempDir::new().unwrap();
    create_test_file(dir.path(), "main.rs", "fn main() {}").unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    encoder.encode(dir.path()).unwrap();

    let json = encoder.to_json_compact().unwrap();
    assert!(!json.contains('\n') || json.matches('\n').count() < 5);
}

#[test]
fn test_encode_register_parser_after_new() {
    let mut encoder = RpgEncoder::new().unwrap();

    encoder.register_parser(Box::new(rpg_encoder::languages::RustParser::new().unwrap()));

    let langs = encoder.languages();
    assert!(langs.contains(&"rust"));
}

#[test]
fn test_encode_languages_list() {
    let encoder = RpgEncoder::new().unwrap();
    let langs = encoder.languages();

    assert!(!langs.is_empty());
    assert!(langs.contains(&"rust"));
}

#[test]
fn test_encode_empty_then_add_file() {
    let dir = TempDir::new().unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    encoder.encode(dir.path()).unwrap();

    create_test_file(dir.path(), "main.rs", "fn main() {}").unwrap();

    let mut encoder2 = RpgEncoder::new().unwrap();
    encoder2.encode(dir.path()).unwrap();

    assert!(encoder2.graph().unwrap().node_count() > 1);
}

#[test]
fn test_encode_same_file_twice() {
    let dir = TempDir::new().unwrap();
    create_test_file(dir.path(), "main.rs", "fn main() {}").unwrap();

    let mut encoder = RpgEncoder::new().unwrap();
    encoder.encode(dir.path()).unwrap();
    let count1 = encoder.graph().unwrap().node_count();

    let mut encoder2 = RpgEncoder::new().unwrap();
    encoder2.encode(dir.path()).unwrap();
    let count2 = encoder2.graph().unwrap().node_count();

    assert_eq!(count1, count2);
}

#[test]
fn test_encode_circular_symlinks() {
    let dir = TempDir::new().unwrap();

    let subdir = dir.path().join("subdir");
    std::fs::create_dir(&subdir).unwrap();

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path(), subdir.join("loop")).ok();

        create_test_file(dir.path(), "main.rs", "fn main() {}").unwrap();

        let mut encoder = RpgEncoder::new().unwrap();
        let result = encoder.encode(dir.path());

        assert!(result.is_ok());
    }
}

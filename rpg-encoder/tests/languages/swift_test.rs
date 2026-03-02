
use rpg_encoder::languages::SwiftParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("swift")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = SwiftParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_imports() {
    let result = parse_fixture("basic.swift");
    assert!(!result.imports.is_empty(), "Should have imports");
}

#[test]
fn test_parse_basic_types() {
    let result = parse_fixture("basic.swift");

    let types: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "struct" || d.kind == "class")
        .collect();

    assert!(!types.is_empty(), "Should have type definitions");
}

#[test]
fn test_parse_basic_functions() {
    let result = parse_fixture("basic.swift");

    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "function" || d.kind == "func")
        .collect();

    assert!(!functions.is_empty(), "Should have function definitions");
}

#[test]
fn test_parse_basic_protocols() {
    let result = parse_fixture("basic.swift");

    let protocols: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "protocol")
        .collect();

    assert!(!protocols.is_empty(), "Should have protocol definitions");
}

#[test]
fn test_parse_ffi() {
    let result = parse_fixture("ffi.swift");

    let exports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Export)
        .collect();

    assert!(!exports.is_empty(), "Should detect Swift FFI exports");
}

#[test]
fn test_language_name() {
    let parser = SwiftParser::new().unwrap();
    assert_eq!(parser.language_name(), "swift");
}

#[test]
fn test_file_extensions() {
    let parser = SwiftParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"swift"));
}

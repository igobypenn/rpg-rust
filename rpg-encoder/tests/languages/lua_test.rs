
use rpg_encoder::languages::LuaParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("lua")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = LuaParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_functions() {
    let result = parse_fixture("basic.lua");

    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "function" || d.kind == "local_function")
        .collect();

    assert!(!functions.is_empty(), "Should have function definitions");
}

#[test]
fn test_parse_basic_calls() {
    let result = parse_fixture("basic.lua");

    assert!(!result.calls.is_empty(), "Should have function calls");
}

#[test]
fn test_parse_ffi() {
    let result = parse_fixture("ffi.lua");

    let imports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Import)
        .collect();

    assert!(!imports.is_empty(), "Should detect LuaJIT FFI imports");
}

#[test]
fn test_language_name() {
    let parser = LuaParser::new().unwrap();
    assert_eq!(parser.language_name(), "lua");
}

#[test]
fn test_file_extensions() {
    let parser = LuaParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"lua"));
}

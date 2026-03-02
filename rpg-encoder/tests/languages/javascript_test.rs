use rpg_encoder::languages::JavaScriptParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("javascript")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = JavaScriptParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_classes() {
    let result = parse_fixture("basic.js");

    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "class")
        .collect();

    assert!(!classes.is_empty(), "Should have class definitions");
    assert!(classes.iter().any(|c| c.name == "Config"));
}

#[test]
fn test_parse_basic_functions() {
    let result = parse_fixture("basic.js");

    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "function")
        .collect();

    assert!(!functions.is_empty(), "Should have function definitions");
}

#[test]
fn test_parse_basic_methods() {
    let result = parse_fixture("basic.js");

    let methods: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "method")
        .collect();

    assert!(!methods.is_empty(), "Should have method definitions");
}

#[test]
fn test_parse_basic_calls() {
    let result = parse_fixture("basic.js");

    assert!(!result.calls.is_empty(), "Should have function calls");
}

#[test]
fn test_parse_native_ffi() {
    let result = parse_fixture("native.js");

    assert!(
        !result.ffi_bindings.is_empty(),
        "Should detect FFI bindings"
    );
}

#[test]
fn test_language_name() {
    let parser = JavaScriptParser::new().unwrap();
    assert_eq!(parser.language_name(), "javascript");
}

#[test]
fn test_file_extensions() {
    let parser = JavaScriptParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"js"));
    assert!(parser.file_extensions().contains(&"mjs"));
    assert!(parser.file_extensions().contains(&"cjs"));
}

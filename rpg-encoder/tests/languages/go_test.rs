
use rpg_encoder::languages::GoParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("go")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = GoParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_imports() {
    let result = parse_fixture("basic.go");

    assert!(!result.imports.is_empty(), "Should have imports");
}

#[test]
fn test_parse_basic_structs() {
    let result = parse_fixture("basic.go");

    let structs: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "struct" || d.kind == "type")
        .collect();

    assert!(!structs.is_empty(), "Should have struct definitions");
    assert!(structs.iter().any(|s| s.name == "Config"));
}

#[test]
fn test_parse_basic_functions() {
    let result = parse_fixture("basic.go");

    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "function" || d.kind == "func")
        .collect();

    assert!(!functions.is_empty(), "Should have function definitions");
    assert!(functions
        .iter()
        .any(|f| f.name == "NewConfig" || f.name == "CreateConfig"));
}

#[test]
fn test_parse_basic_methods() {
    let result = parse_fixture("basic.go");

    let methods: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "method")
        .collect();

    assert!(!methods.is_empty(), "Should have method definitions");
}

#[test]
fn test_parse_basic_calls() {
    let result = parse_fixture("basic.go");

    assert!(!result.calls.is_empty(), "Should have function calls");
}

#[test]
fn test_parse_cgo_exports() {
    let result = parse_fixture("cgo.go");

    let exports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Export)
        .collect();

    assert!(!exports.is_empty(), "Should detect cgo exports");
    assert!(exports.iter().any(|e| e.symbol == "GoExportedFunction"));
}

#[test]
fn test_parse_cgo_imports() {
    let result = parse_fixture("cgo.go");

    let imports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Import)
        .collect();

    assert!(!imports.is_empty(), "Should detect cgo imports");
}

#[test]
fn test_language_name() {
    let parser = GoParser::new().unwrap();
    assert_eq!(parser.language_name(), "go");
}

#[test]
fn test_file_extensions() {
    let parser = GoParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"go"));
}

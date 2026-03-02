use rpg_encoder::languages::RustParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("rust")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = RustParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_imports() {
    let result = parse_fixture("basic.rs");

    let std_imports: Vec<_> = result
        .imports
        .iter()
        .filter(|i| i.module_path.starts_with("std"))
        .collect();

    assert!(!std_imports.is_empty(), "Should have std imports");
}

#[test]
fn test_parse_basic_structs() {
    let result = parse_fixture("basic.rs");

    let structs: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "struct")
        .collect();

    assert!(!structs.is_empty(), "Should have struct definitions");
    assert!(structs.iter().any(|s| s.name == "AppConfig"));
}

#[test]
fn test_parse_basic_functions() {
    let result = parse_fixture("basic.rs");

    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "fn")
        .collect();

    assert!(!functions.is_empty(), "Should have function definitions");
    assert!(functions.iter().any(|f| f.name == "create_config"));
}

#[test]
fn test_parse_basic_impls() {
    let result = parse_fixture("basic.rs");

    let impls: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "impl")
        .collect();

    assert!(!impls.is_empty(), "Should have impl blocks");
}

#[test]
fn test_parse_types_structs() {
    let result = parse_fixture("types.rs");

    let structs: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "struct")
        .collect();

    assert!(structs.iter().any(|s| s.name == "User"));
    assert!(structs.iter().any(|s| s.name == "Cache"));
}

#[test]
fn test_parse_types_enums() {
    let result = parse_fixture("types.rs");

    let enums: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "enum")
        .collect();

    assert!(enums.iter().any(|e| e.name == "Status"));
}

#[test]
fn test_parse_types_traits() {
    let result = parse_fixture("types.rs");

    let traits: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "trait")
        .collect();

    assert!(traits.iter().any(|t| t.name == "Repository"));
}

#[test]
fn test_parse_ffi_bindings() {
    let result = parse_fixture("ffi.rs");

    // Should have FFI bindings (both exports and imports)
    assert!(
        !result.ffi_bindings.is_empty(),
        "Should detect FFI bindings"
    );

    // Should have exports from #[no_mangle]
    let exports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Export)
        .collect();

    assert!(!exports.is_empty(), "Should detect exports");
}

#[test]
fn test_parse_type_refs() {
    let result = parse_fixture("basic.rs");

    assert!(!result.type_refs.is_empty(), "Should have type references");
}

#[test]
fn test_parse_calls() {
    let result = parse_fixture("basic.rs");

    assert!(!result.calls.is_empty(), "Should have function calls");
}

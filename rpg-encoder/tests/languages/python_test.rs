use rpg_encoder::languages::PythonParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("python")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = PythonParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_imports() {
    let result = parse_fixture("basic.py");

    assert!(!result.imports.is_empty(), "Should have imports");
}

#[test]
fn test_parse_basic_classes() {
    let result = parse_fixture("basic.py");

    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "class")
        .collect();

    assert!(!classes.is_empty(), "Should have class definitions");
    assert!(classes.iter().any(|c| c.name == "DataProcessor"));
}

#[test]
fn test_parse_basic_functions() {
    let result = parse_fixture("basic.py");

    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "def" && d.parent.is_none())
        .collect();

    assert!(!functions.is_empty(), "Should have function definitions");
}

#[test]
fn test_parse_basic_methods() {
    let result = parse_fixture("basic.py");

    let methods: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "def" && d.parent.is_some())
        .collect();

    assert!(!methods.is_empty(), "Should have method definitions");
}

#[test]
fn test_parse_basic_calls() {
    let result = parse_fixture("basic.py");

    assert!(!result.calls.is_empty(), "Should have function calls");
}

#[test]
fn test_parse_ffi_ctypes() {
    let result = parse_fixture("ffi_ctypes.py");

    let imports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Import)
        .collect();

    assert!(!imports.is_empty(), "Should detect ctypes FFI imports");
}

#[test]
fn test_language_name() {
    let parser = PythonParser::new().unwrap();
    assert_eq!(parser.language_name(), "python");
}

#[test]
fn test_file_extensions() {
    let parser = PythonParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"py"));
}

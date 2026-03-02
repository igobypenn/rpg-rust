use rpg_encoder::languages::JavaParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("java")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = JavaParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_imports() {
    let result = parse_fixture("Basic.java");

    assert!(!result.imports.is_empty(), "Should have imports");
}

#[test]
fn test_parse_basic_classes() {
    let result = parse_fixture("Basic.java");

    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "class")
        .collect();

    assert!(!classes.is_empty(), "Should have class definitions");
}

#[test]
fn test_parse_basic_interfaces() {
    let result = parse_fixture("Basic.java");

    let interfaces: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "interface")
        .collect();

    assert!(!interfaces.is_empty(), "Should have interface definitions");
}

#[test]
fn test_parse_basic_methods() {
    let result = parse_fixture("Basic.java");

    let methods: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "method")
        .collect();

    assert!(!methods.is_empty(), "Should have method definitions");
}

#[test]
fn test_parse_basic_calls() {
    let result = parse_fixture("Basic.java");

    assert!(!result.calls.is_empty(), "Should have method calls");
}

#[test]
fn test_parse_jni() {
    let result = parse_fixture("Jni.java");

    let imports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Import)
        .collect();

    assert!(!imports.is_empty(), "Should detect JNI imports");
}

#[test]
fn test_parse_package() {
    let result = parse_fixture("Basic.java");

    let has_package = result
        .definitions
        .iter()
        .any(|d| d.metadata.contains_key("package"));

    assert!(has_package, "Should extract package information");
}

#[test]
fn test_language_name() {
    let parser = JavaParser::new().unwrap();
    assert_eq!(parser.language_name(), "java");
}

#[test]
fn test_file_extensions() {
    let parser = JavaParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"java"));
}

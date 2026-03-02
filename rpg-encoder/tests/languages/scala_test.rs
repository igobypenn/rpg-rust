
use rpg_encoder::languages::ScalaParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("scala")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = ScalaParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_imports() {
    let result = parse_fixture("Basic.scala");

    assert!(!result.imports.is_empty(), "Should have imports");
}

#[test]
fn test_parse_basic_packages() {
    let result = parse_fixture("Basic.scala");

    let has_package = result
        .definitions
        .iter()
        .any(|d| d.metadata.contains_key("package") || d.kind == "package");

    assert!(has_package, "Should have package definitions");
}

#[test]
fn test_parse_basic_classes() {
    let result = parse_fixture("Basic.scala");

    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "class")
        .collect();

    assert!(!classes.is_empty(), "Should have class definitions");
}

#[test]
fn test_parse_basic_traits() {
    let result = parse_fixture("Basic.scala");

    let traits: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "trait")
        .collect();

    assert!(!traits.is_empty(), "Should have trait definitions");
}

#[test]
fn test_parse_basic_objects() {
    let result = parse_fixture("Basic.scala");

    let objects: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "object")
        .collect();

    assert!(!objects.is_empty(), "Should have object definitions");
}

#[test]
fn test_parse_basic_functions() {
    let result = parse_fixture("Basic.scala");

    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "function" || d.kind == "def")
        .collect();

    assert!(!functions.is_empty(), "Should have function definitions");
}

#[test]
fn test_parse_basic_case_classes() {
    let result = parse_fixture("Basic.scala");

    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "class")
        .collect();

    assert!(
        !classes.is_empty(),
        "Should have class definitions including case classes"
    );
}

#[test]
fn test_parse_ffi_jna() {
    let result = parse_fixture("FFI.scala");

    let imports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Import)
        .collect();

    assert!(!imports.is_empty(), "Should detect JNA/JNI imports");
}

#[test]
fn test_language_name() {
    let parser = ScalaParser::new().unwrap();
    assert_eq!(parser.language_name(), "scala");
}

#[test]
fn test_file_extensions() {
    let parser = ScalaParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"scala"));
    assert!(parser.file_extensions().contains(&"sc"));
}

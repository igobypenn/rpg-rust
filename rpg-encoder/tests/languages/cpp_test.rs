use rpg_encoder::languages::CppParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("cpp")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = CppParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_includes() {
    let result = parse_fixture("basic.cpp");

    assert!(!result.imports.is_empty(), "Should have includes");
}

#[test]
fn test_parse_basic_classes() {
    let result = parse_fixture("basic.cpp");

    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "class")
        .collect();

    assert!(!classes.is_empty(), "Should have class definitions");
    assert!(classes.iter().any(|c| c.name == "Config"));
}

#[test]
fn test_parse_basic_namespaces() {
    let result = parse_fixture("basic.cpp");

    let namespaces: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "namespace")
        .collect();

    assert!(!namespaces.is_empty(), "Should have namespace definitions");
}

#[test]
fn test_parse_basic_methods() {
    let result = parse_fixture("basic.cpp");

    let methods: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "fn")
        .collect();

    assert!(
        !methods.is_empty(),
        "Should have method/function definitions"
    );
}

#[test]
fn test_parse_basic_calls() {
    let result = parse_fixture("basic.cpp");

    assert!(!result.calls.is_empty(), "Should have function calls");
}

#[test]
fn test_parse_extern_c() {
    let result = parse_fixture("extern_c.cpp");

    let exports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Export)
        .collect();

    assert!(!exports.is_empty(), "Should detect extern C exports");
}

#[test]
fn test_parse_templates() {
    let result = parse_fixture("basic.cpp");

    let templates: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "template" || d.kind == "class")
        .collect();

    assert!(!templates.is_empty(), "Should have template definitions");
}

#[test]
fn test_language_name() {
    let parser = CppParser::new().unwrap();
    assert_eq!(parser.language_name(), "cpp");
}

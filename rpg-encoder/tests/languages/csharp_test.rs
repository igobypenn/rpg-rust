use rpg_encoder::languages::CSharpParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("csharp")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = CSharpParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_namespaces() {
    let result = parse_fixture("Basic.cs");

    // Namespaces are stored in metadata, not as separate definitions
    let has_namespace = result
        .definitions
        .iter()
        .any(|d| d.metadata.contains_key("namespace"));

    assert!(
        has_namespace,
        "Should have namespace metadata in definitions"
    );
}

#[test]
fn test_parse_basic_classes() {
    let result = parse_fixture("Basic.cs");

    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "class")
        .collect();

    assert!(!classes.is_empty(), "Should have class definitions");
    assert!(classes.iter().any(|c| c.name == "Config"));
}

#[test]
fn test_parse_basic_interfaces() {
    let result = parse_fixture("Basic.cs");

    let interfaces: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "interface")
        .collect();

    assert!(!interfaces.is_empty(), "Should have interface definitions");
}

#[test]
fn test_parse_basic_methods() {
    let result = parse_fixture("Basic.cs");

    let methods: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "method")
        .collect();

    assert!(!methods.is_empty(), "Should have method definitions");
}

#[test]
fn test_parse_basic_properties() {
    let result = parse_fixture("Basic.cs");

    let properties: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "property")
        .collect();

    assert!(!properties.is_empty(), "Should have property definitions");
}

#[test]
fn test_parse_basic_records() {
    let result = parse_fixture("Basic.cs");

    let records: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "record")
        .collect();

    assert!(!records.is_empty(), "Should have record definitions");
}

#[test]
fn test_parse_basic_calls() {
    let result = parse_fixture("Basic.cs");

    assert!(!result.calls.is_empty(), "Should have method calls");
}

#[test]
fn test_parse_pinvoke() {
    let result = parse_fixture("PInvoke.cs");

    let imports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Import)
        .collect();

    assert!(!imports.is_empty(), "Should detect P/Invoke imports");
}

#[test]
fn test_parse_exports() {
    let result = parse_fixture("PInvoke.cs");

    let exports: Vec<_> = result
        .ffi_bindings
        .iter()
        .filter(|b| b.kind == rpg_encoder::languages::ffi::FfiKind::Export)
        .collect();

    assert!(!exports.is_empty(), "Should detect NativeAOT exports");
}

#[test]
fn test_language_name() {
    let parser = CSharpParser::new().unwrap();
    assert_eq!(parser.language_name(), "csharp");
}

#[test]
fn test_file_extensions() {
    let parser = CSharpParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"cs"));
}

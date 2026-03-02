
use rpg_encoder::languages::CParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("c")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = CParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_includes() {
    let result = parse_fixture("basic.c");

    assert!(!result.imports.is_empty(), "Should have includes");
}

#[test]
fn test_parse_basic_structs() {
    let result = parse_fixture("basic.c");

    let structs: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "struct" || d.kind == "typedef")
        .collect();

    assert!(!structs.is_empty(), "Should have struct definitions");
}

#[test]
fn test_parse_basic_functions() {
    let result = parse_fixture("basic.c");

    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "fn")
        .collect();

    assert!(
        !functions.is_empty(),
        "Should have function definitions, got: {:?}",
        result
            .definitions
            .iter()
            .map(|d| (&d.kind, &d.name))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_parse_basic_calls() {
    let result = parse_fixture("basic.c");

    assert!(!result.calls.is_empty(), "Should have function calls");
}

#[test]
fn test_parse_typedef() {
    let result = parse_fixture("basic.c");

    let typedefs: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "typedef")
        .collect();

    assert!(!typedefs.is_empty(), "Should have typedef definitions");
}

#[test]
fn test_language_name() {
    let parser = CParser::new().unwrap();
    assert_eq!(parser.language_name(), "c");
}

#[test]
fn test_file_extensions() {
    let parser = CParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"c"));
    assert!(parser.file_extensions().contains(&"h"));
}

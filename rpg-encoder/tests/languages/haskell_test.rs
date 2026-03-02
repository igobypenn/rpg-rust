
use rpg_encoder::languages::HaskellParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("haskell")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = HaskellParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_datatypes() {
    let result = parse_fixture("Basic.hs");

    let datatypes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "data")
        .collect();

    assert!(!datatypes.is_empty(), "Should have data type definitions");
}

#[test]
fn test_parse_basic_functions() {
    let result = parse_fixture("Basic.hs");

    let functions: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "function" || d.kind == "signature")
        .collect();

    assert!(
        !functions.is_empty(),
        "Should have function/signature definitions, got: {:?}",
        result
            .definitions
            .iter()
            .map(|d| (&d.kind, &d.name))
            .collect::<Vec<_>>()
    );
}

#[test]
fn test_parse_basic_typeclasses() {
    let result = parse_fixture("Basic.hs");

    let typeclasses: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "class")
        .collect();

    assert!(!typeclasses.is_empty(), "Should have typeclass definitions");
}

#[test]
fn test_parse_ffi() {
    let result = parse_fixture("FFI.hs");

    let ffi_bindings = &result.ffi_bindings;
    assert!(
        !ffi_bindings.is_empty(),
        "Should detect Haskell FFI bindings"
    );
}

#[test]
fn test_language_name() {
    let parser = HaskellParser::new().unwrap();
    assert_eq!(parser.language_name(), "haskell");
}

#[test]
fn test_file_extensions() {
    let parser = HaskellParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"hs"));
}

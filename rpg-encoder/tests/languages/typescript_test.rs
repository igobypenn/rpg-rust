
use rpg_encoder::languages::TypeScriptParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("typescript")
}

fn load_fixture(name: &str) -> String {
    std::fs::read_to_string(fixtures_dir().join(name))
        .unwrap_or_else(|_| panic!("Failed to load fixture: {}", name))
}

fn parse_fixture(name: &str) -> rpg_encoder::parser::ParseResult {
    let source = load_fixture(name);
    let parser = TypeScriptParser::new().expect("Failed to create parser");
    let path = fixtures_dir().join(name);
    parser
        .parse(&source, &path)
        .expect("Failed to parse fixture")
}

#[test]
fn test_parse_basic_interfaces() {
    let result = parse_fixture("basic.ts");

    let interfaces: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "interface")
        .collect();

    assert!(!interfaces.is_empty(), "Should have interface definitions");
    assert!(interfaces.iter().any(|i| i.name == "IConfig"));
}

#[test]
fn test_parse_basic_classes() {
    let result = parse_fixture("basic.ts");

    let classes: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "class")
        .collect();

    assert!(!classes.is_empty(), "Should have class definitions");
    assert!(classes.iter().any(|c| c.name == "Config"));
}

#[test]
fn test_parse_basic_type_aliases() {
    let result = parse_fixture("basic.ts");

    let type_aliases: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "type_alias" || d.kind == "type")
        .collect();

    assert!(
        !type_aliases.is_empty(),
        "Should have type alias definitions"
    );
}

#[test]
fn test_parse_generics() {
    let result = parse_fixture("generics.ts");

    let generics: Vec<_> = result
        .definitions
        .iter()
        .filter(|d| d.kind == "interface" || d.kind == "class")
        .collect();

    assert!(generics.iter().any(|g| g.name == "Repository"));
}

#[test]
fn test_parse_basic_calls() {
    let result = parse_fixture("basic.ts");

    assert!(!result.calls.is_empty(), "Should have function calls");
}

#[test]
fn test_language_name() {
    let parser = TypeScriptParser::new().unwrap();
    assert_eq!(parser.language_name(), "typescript");
}

#[test]
fn test_file_extensions() {
    let parser = TypeScriptParser::new().unwrap();
    assert!(parser.file_extensions().contains(&"ts"));
    assert!(parser.file_extensions().contains(&"tsx"));
}


use rpg_encoder::languages::SwiftParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_swift_parse_empty_file() {
    let parser = SwiftParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.swift")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_swift_parse_syntax_error_recovery() {
    let parser = SwiftParser::new().unwrap();
    let source = r#"
func broken( {
    missing closing paren

func working() {
    print("hello")
}
"#;

    let result = parser.parse(source, &PathBuf::from("broken.swift"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

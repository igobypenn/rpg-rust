use rpg_encoder::languages::GoParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_go_parse_empty_file() {
    let parser = GoParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.go")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_go_parse_syntax_error_recovery() {
    let parser = GoParser::new().unwrap();
    let source = r#"
package main

func broken( {
    missing closing paren

func working() {
    println("hello")
}
"#;

    let result = parser.parse(source, &PathBuf::from("broken.go"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

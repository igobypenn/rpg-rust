use rpg_encoder::languages::RustParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_rust_parse_empty_file() {
    let parser = RustParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.rs")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_rust_parse_syntax_error_recovery() {
    let parser = RustParser::new().unwrap();
    let source = r#"
fn broken(
    missing closing paren
    
fn working() {
    println!("hello");
}
"#;

    let result = parser.parse(source, &PathBuf::from("broken.rs"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

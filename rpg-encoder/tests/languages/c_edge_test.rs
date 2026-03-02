use rpg_encoder::languages::CParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_c_parse_empty_file() {
    let parser = CParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.c")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_c_parse_syntax_error_recovery() {
    let parser = CParser::new().unwrap();
    let source = r#"
void broken( {
    missing closing paren

void working() {
    printf("hello");
}
"#;

    let result = parser.parse(source, &PathBuf::from("broken.c"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

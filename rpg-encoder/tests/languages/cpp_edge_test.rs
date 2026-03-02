
use rpg_encoder::languages::CppParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_cpp_parse_empty_file() {
    let parser = CppParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.cpp")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_cpp_parse_syntax_error_recovery() {
    let parser = CppParser::new().unwrap();
    let source = r#"
void broken( {
    missing closing paren

void working() {
    std::cout << "hello";
}
"#;

    let result = parser.parse(source, &PathBuf::from("broken.cpp"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

use rpg_encoder::languages::JavaParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_java_parse_empty_file() {
    let parser = JavaParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("Empty.java")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_java_parse_syntax_error_recovery() {
    let parser = JavaParser::new().unwrap();
    let source = r#"
public class Broken {
    public void broken( {
        missing closing paren

    public void working() {
        System.out.println("hello");
    }
}
"#;

    let result = parser.parse(source, &PathBuf::from("Broken.java"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

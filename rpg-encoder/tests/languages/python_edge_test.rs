
use rpg_encoder::languages::PythonParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_python_parse_empty_file() {
    let parser = PythonParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.py")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_python_parse_syntax_error_recovery() {
    let parser = PythonParser::new().unwrap();
    let source = r#"
def broken(
    missing colon

def working():
    print("hello")
"#;

    let result = parser.parse(source, &PathBuf::from("broken.py"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

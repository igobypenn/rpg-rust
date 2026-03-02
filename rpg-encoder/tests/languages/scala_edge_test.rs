use rpg_encoder::languages::ScalaParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_scala_parse_empty_file() {
    let parser = ScalaParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.scala")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_scala_parse_syntax_error_recovery() {
    let parser = ScalaParser::new().unwrap();
    let source = r#"
def broken( {
    missing closing paren

def working(): Unit = {
    println("hello")
}
"#;

    let result = parser.parse(source, &PathBuf::from("broken.scala"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

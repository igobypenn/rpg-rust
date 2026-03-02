
use rpg_encoder::languages::JavaScriptParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_javascript_parse_empty_file() {
    let parser = JavaScriptParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.js")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_javascript_parse_syntax_error_recovery() {
    let parser = JavaScriptParser::new().unwrap();
    let source = r#"
function broken( {
    missing closing paren

function working() {
    console.log("hello");
}
"#;

    let result = parser.parse(source, &PathBuf::from("broken.js"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}


use rpg_encoder::languages::TypeScriptParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_typescript_parse_empty_file() {
    let parser = TypeScriptParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.ts")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_typescript_parse_syntax_error_recovery() {
    let parser = TypeScriptParser::new().unwrap();
    let source = r#"
function broken( {
    missing closing paren

function working(): void {
    console.log("hello");
}
"#;

    let result = parser.parse(source, &PathBuf::from("broken.ts"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

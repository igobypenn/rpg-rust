
use rpg_encoder::languages::HaskellParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_haskell_parse_empty_file() {
    let parser = HaskellParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.hs")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_haskell_parse_syntax_error_recovery() {
    let parser = HaskellParser::new().unwrap();
    let source = r#"
broken :: String -> 
  missing type

working :: String -> String
working x = x
"#;

    let result = parser.parse(source, &PathBuf::from("broken.hs"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

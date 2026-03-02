
use rpg_encoder::languages::LuaParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_lua_parse_empty_file() {
    let parser = LuaParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.lua")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_lua_parse_syntax_error_recovery() {
    let parser = LuaParser::new().unwrap();
    let source = r#"
function broken(
    missing closing paren

function working()
    print("hello")
end
"#;

    let result = parser.parse(source, &PathBuf::from("broken.lua"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

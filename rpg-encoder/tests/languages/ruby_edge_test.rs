
use rpg_encoder::languages::RubyParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_ruby_parse_empty_file() {
    let parser = RubyParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("empty.rb")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_ruby_parse_syntax_error_recovery() {
    let parser = RubyParser::new().unwrap();
    let source = r#"
def broken(
  missing closing paren

def working
  puts "hello"
end
"#;

    let result = parser.parse(source, &PathBuf::from("broken.rb"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

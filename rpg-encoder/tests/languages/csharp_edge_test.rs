use rpg_encoder::languages::CSharpParser;
use rpg_encoder::parser::LanguageParser;
use std::path::PathBuf;

#[test]
fn test_csharp_parse_empty_file() {
    let parser = CSharpParser::new().unwrap();
    let result = parser.parse("", &PathBuf::from("Empty.cs")).unwrap();

    assert!(result.definitions.is_empty());
    assert!(result.imports.is_empty());
}

#[test]
fn test_csharp_parse_syntax_error_recovery() {
    let parser = CSharpParser::new().unwrap();
    let source = r#"
public class Broken {
    public void Broken( {
        missing closing paren

    public void Working() {
        Console.WriteLine("hello");
    }
}
"#;

    let result = parser.parse(source, &PathBuf::from("Broken.cs"));
    assert!(
        result.is_ok(),
        "Parser should handle syntax errors gracefully"
    );
}

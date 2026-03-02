use rpg_encoder::languages::RustParser;
use rpg_encoder::parser::{LanguageParser, ParserRegistry};
use std::path::PathBuf;

#[test]
fn test_registry_empty() {
    let registry = ParserRegistry::new();
    assert!(registry.languages().is_empty());
}

#[test]
fn test_registry_register_single() {
    let mut registry = ParserRegistry::new();
    let parser = RustParser::new().expect("Failed to create parser");
    registry.register(Box::new(parser));

    assert_eq!(registry.languages().len(), 1);
    assert_eq!(registry.languages()[0], "rust");
}

#[test]
fn test_registry_register_multiple() {
    let mut registry = ParserRegistry::new();
    registry.register(Box::new(RustParser::new().unwrap()));

    assert_eq!(registry.languages().len(), 1);
}

#[test]
fn test_registry_get_parser_by_extension() {
    let mut registry = ParserRegistry::new();
    registry.register(Box::new(RustParser::new().unwrap()));

    let path = PathBuf::from("src/main.rs");
    let parser = registry.get_parser(&path);

    assert!(parser.is_some());
    assert_eq!(parser.unwrap().language_name(), "rust");
}

#[test]
fn test_registry_get_parser_unknown_extension() {
    let mut registry = ParserRegistry::new();
    registry.register(Box::new(RustParser::new().unwrap()));

    let path = PathBuf::from("src/main.xyz");
    let parser = registry.get_parser(&path);

    assert!(parser.is_none());
}

#[test]
fn test_registry_has_parser_true() {
    let mut registry = ParserRegistry::new();
    registry.register(Box::new(RustParser::new().unwrap()));

    let path = PathBuf::from("src/main.rs");
    assert!(registry.has_parser(&path));
}

#[test]
fn test_registry_has_parser_false() {
    let registry = ParserRegistry::new();

    let path = PathBuf::from("src/main.rs");
    assert!(!registry.has_parser(&path));
}

#[test]
fn test_registry_no_extension_file() {
    let mut registry = ParserRegistry::new();
    registry.register(Box::new(RustParser::new().unwrap()));

    let path = PathBuf::from("Makefile");
    let parser = registry.get_parser(&path);

    assert!(parser.is_none());
}

#[test]
fn test_registry_languages_list() {
    let mut registry = ParserRegistry::new();
    registry.register(Box::new(RustParser::new().unwrap()));

    let langs = registry.languages();
    assert_eq!(langs, vec!["rust"]);
}

#[test]
fn test_parser_can_parse_true() {
    let parser = RustParser::new().unwrap();
    let path = PathBuf::from("main.rs");
    assert!(parser.can_parse(&path));
}

#[test]
fn test_parser_can_parse_false() {
    let parser = RustParser::new().unwrap();
    let path = PathBuf::from("main.py");
    assert!(!parser.can_parse(&path));
}

#[test]
fn test_parser_default_category() {
    use rpg_encoder::NodeCategory;
    let parser = RustParser::new().unwrap();
    assert_eq!(parser.default_category(), NodeCategory::File);
}

#[test]
fn test_parse_result_new() {
    use rpg_encoder::parser::ParseResult;
    let path = PathBuf::from("test.rs");
    let result = ParseResult::new(path.clone());

    assert_eq!(result.file_path, path);
    assert!(result.imports.is_empty());
    assert!(result.definitions.is_empty());
    assert!(result.calls.is_empty());
    assert!(result.type_refs.is_empty());
    assert!(result.references.is_empty());
    assert!(result.ffi_bindings.is_empty());
}

#[test]
fn test_import_info_builders() {
    use rpg_encoder::parser::ImportInfo;
    use rpg_encoder::SourceLocation;
    use std::path::PathBuf;

    let import = ImportInfo::new("std::collections")
        .with_names(vec!["HashMap".to_string(), "HashSet".to_string()])
        .with_glob(false)
        .with_location(SourceLocation::new(PathBuf::from("test.rs"), 1, 1, 1, 30))
        .with_metadata(
            "alias",
            serde_json::Value::String("collections".to_string()),
        );

    assert_eq!(import.module_path, "std::collections");
    assert_eq!(import.imported_names, vec!["HashMap", "HashSet"]);
    assert!(!import.is_glob);
    assert!(import.location.is_some());
    assert!(import.metadata.contains_key("alias"));
}

#[test]
fn test_definition_info_builders() {
    use rpg_encoder::parser::DefinitionInfo;
    use rpg_encoder::SourceLocation;
    use std::path::PathBuf;

    let def = DefinitionInfo::new("fn", "main")
        .with_location(SourceLocation::new(PathBuf::from("test.rs"), 1, 1, 5, 2))
        .with_parent("module")
        .with_signature("fn main() -> Result<()>")
        .with_visibility(true)
        .with_doc("Main entry point")
        .with_metadata("async", serde_json::Value::Bool(true));

    assert_eq!(def.kind, "fn");
    assert_eq!(def.name, "main");
    assert!(def.location.is_some());
    assert_eq!(def.parent, Some("module".to_string()));
    assert_eq!(def.signature, Some("fn main() -> Result<()>".to_string()));
    assert!(def.is_public);
    assert_eq!(def.doc, Some("Main entry point".to_string()));
    assert!(def.metadata.contains_key("async"));
}

#[test]
fn test_call_info_factories() {
    use rpg_encoder::parser::{CallInfo, CallKind};
    use rpg_encoder::SourceLocation;
    use std::path::PathBuf;

    let direct = CallInfo::new("main", "helper");
    assert_eq!(direct.caller, "main");
    assert_eq!(direct.callee, "helper");
    assert_eq!(direct.call_kind, CallKind::Direct);
    assert!(direct.receiver.is_none());

    let method = CallInfo::method("main", "Config", "load");
    assert_eq!(method.caller, "main");
    assert_eq!(method.callee, "load");
    assert_eq!(method.receiver, Some("Config".to_string()));
    assert_eq!(method.call_kind, CallKind::Method);

    let associated = CallInfo::associated("main", "Utils", "process");
    assert_eq!(associated.callee, "Utils::process");
    assert_eq!(associated.call_kind, CallKind::Associated);

    let with_loc = CallInfo::new("main", "helper").with_location(SourceLocation::new(
        PathBuf::from("test.rs"),
        10,
        5,
        10,
        15,
    ));
    assert!(with_loc.location.is_some());
}

#[test]
fn test_type_ref_info_factories() {
    use rpg_encoder::parser::{TypeRefInfo, TypeRefKind};

    let param = TypeRefInfo::param("main", "String");
    assert_eq!(param.source, "main");
    assert_eq!(param.type_name, "String");
    assert_eq!(param.ref_kind, TypeRefKind::Parameter);

    let ret = TypeRefInfo::ret("main", "Result");
    assert_eq!(ret.ref_kind, TypeRefKind::Return);

    let field = TypeRefInfo::field("Config", "HashMap");
    assert_eq!(field.ref_kind, TypeRefKind::Field);

    let custom = TypeRefInfo::new("main", "Vec").with_kind(TypeRefKind::GenericArg);
    assert_eq!(custom.ref_kind, TypeRefKind::GenericArg);
}

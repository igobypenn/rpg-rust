//! Property tests for parser operations

use super::generators::*;
use proptest::prelude::*;
use rpg_encoder::languages::RustParser;
use rpg_encoder::parser::LanguageParser;
use std::path::Path;

proptest! {
    #[test]
    fn parse_never_panics(source in arbitrary_source_code()) {
        let parser = match RustParser::new() {
            Ok(p) => p,
            Err(_) => return Err(proptest::test_runner::TestCaseError::Reject("Parser init failed".into())),
        };
        let path = Path::new("test.rs");

        let _ = parser.parse(&source, path);
    }

    #[test]
    fn parse_valid_function(code in rust_function_code()) {
        let parser = match RustParser::new() {
            Ok(p) => p,
            Err(_) => return Err(proptest::test_runner::TestCaseError::Reject("Parser init failed".into())),
        };
        let path = Path::new("test.rs");

        let result = parser.parse(&code, path);
        prop_assert!(result.is_ok());

        if let Ok(parse_result) = result {
            prop_assert!(
                !parse_result.definitions.is_empty(),
                "Expected at least one definition from function"
            );
        }
    }

    #[test]
    fn parse_valid_struct(code in rust_struct_code()) {
        let parser = match RustParser::new() {
            Ok(p) => p,
            Err(_) => return Err(proptest::test_runner::TestCaseError::Reject("Parser init failed".into())),
        };
        let path = Path::new("test.rs");

        let result = parser.parse(&code, path);
        prop_assert!(result.is_ok());

        if let Ok(parse_result) = result {
            prop_assert!(
                !parse_result.definitions.is_empty(),
                "Expected at least one definition from struct"
            );
        }
    }

    #[test]
    fn definition_count_bounded(
        functions in proptest::collection::vec(rust_function_code(), 0..15)
    ) {
        let parser = match RustParser::new() {
            Ok(p) => p,
            Err(_) => return Err(proptest::test_runner::TestCaseError::Reject("Parser init failed".into())),
        };
        let source = functions.join("\n\n");
        let path = Path::new("test.rs");

        let result = parser.parse(&source, path);
        if let Ok(parse_result) = result {
            let function_count = parse_result.definitions.iter()
                .filter(|d| d.kind == "fn")
                .count();

            prop_assert!(
                function_count <= functions.len(),
                "Function count {} exceeds generated {}",
                function_count,
                functions.len()
            );
        }
    }

    #[test]
    fn parse_empty_is_valid(_ in Just(())) {
        let parser = match RustParser::new() {
            Ok(p) => p,
            Err(_) => return Err(proptest::test_runner::TestCaseError::Reject("Parser init failed".into())),
        };
        let path = Path::new("test.rs");

        let result = parser.parse("", path);
        prop_assert!(result.is_ok());

        if let Ok(parse_result) = result {
            prop_assert_eq!(parse_result.definitions.len(), 0);
            prop_assert_eq!(parse_result.imports.len(), 0);
        }
    }

    #[test]
    fn unicode_source_is_handled(source in "\\PC*") {
        let parser = match RustParser::new() {
            Ok(p) => p,
            Err(_) => return Err(proptest::test_runner::TestCaseError::Reject("Parser init failed".into())),
        };
        let path = Path::new("test.rs");

        let _ = parser.parse(&source, path);
    }
}

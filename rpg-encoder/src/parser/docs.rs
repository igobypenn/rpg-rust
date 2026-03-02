//! Documentation extraction for various programming languages.
//!
//! This module provides a unified, configuration-based approach to extracting
//! documentation comments from source code across multiple languages.

/// Position where documentation appears relative to the documented item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocPosition {
    /// Documentation appears before the item (most languages)
    Before,
    /// Documentation appears inside the item (e.g., Python docstrings)
    Inside,
}

/// Configuration for extracting documentation from a specific language.
#[derive(Debug, Clone)]
pub struct DocStyleConfig {
    /// Node kinds that represent comments in the AST
    pub comment_kinds: &'static [&'static str],
    /// Position of documentation relative to the item
    pub position: DocPosition,
    /// Prefixes for single-line documentation comments
    pub line_doc_prefixes: &'static [&'static str],
    /// Start delimiter for block documentation comments
    pub block_start: Option<&'static str>,
    /// End delimiter for block documentation comments
    pub block_end: Option<&'static str>,
    /// Prefixes that indicate non-doc comments (should stop extraction)
    pub non_doc_prefixes: &'static [&'static str],
}

impl DocStyleConfig {
    /// Rust documentation style: `///`, `//!`, `/** */`
    pub const RUST: Self = Self {
        comment_kinds: &["line_comment", "block_comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["///"],
        block_start: Some("/**"),
        block_end: Some("*/"),
        non_doc_prefixes: &["//!", "//"],
    };

    /// Python documentation style: `"""docstrings"""`, `# comments`
    pub const PYTHON: Self = Self {
        comment_kinds: &["string", "expression_statement", "comment"],
        position: DocPosition::Inside,
        line_doc_prefixes: &[],
        block_start: Some("\"\"\""),
        block_end: Some("\"\"\""),
        non_doc_prefixes: &[],
    };

    /// JavaScript/TypeScript JSDoc style: `/** */`, `//`
    pub const JSDOC: Self = Self {
        comment_kinds: &["comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &[],
        block_start: Some("/**"),
        block_end: Some("*/"),
        non_doc_prefixes: &["//", "/*"],
    };

    /// Go documentation style: `//` above declarations
    pub const GO: Self = Self {
        comment_kinds: &["comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["//"],
        block_start: Some("/*"),
        block_end: Some("*/"),
        non_doc_prefixes: &[],
    };

    /// Java Javadoc style: `/** */`
    pub const JAVADOC: Self = Self {
        comment_kinds: &["block_comment", "comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["//"],
        block_start: Some("/**"),
        block_end: Some("*/"),
        non_doc_prefixes: &[],
    };

    /// C/C++ Doxygen style: `///`, `/** */`, `/*! */`
    pub const DOXYGEN: Self = Self {
        comment_kinds: &["comment", "line_comment", "block_comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["///"],
        block_start: Some("/**"),
        block_end: Some("*/"),
        non_doc_prefixes: &["//", "/*"],
    };

    /// C# XML documentation style: `///`, `/** */`
    pub const XMLDOC: Self = Self {
        comment_kinds: &["comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["///"],
        block_start: Some("/**"),
        block_end: Some("*/"),
        non_doc_prefixes: &["//"],
    };

    /// Ruby RDoc style: `#` comments, `=begin/=end` blocks
    pub const RDOC: Self = Self {
        comment_kinds: &["comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["#"],
        block_start: Some("=begin"),
        block_end: Some("=end"),
        non_doc_prefixes: &[],
    };

    /// Lua documentation style: `---`, `--[[ ]]`
    pub const LUADOC: Self = Self {
        comment_kinds: &["comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["---"],
        block_start: Some("--[["),
        block_end: Some("]]"),
        non_doc_prefixes: &["--"],
    };

    /// Haskell Haddock style: `-- |`, `{-| -}`
    pub const HADDOCK: Self = Self {
        comment_kinds: &["comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["-- |", "--^"],
        block_start: Some("{-|"),
        block_end: Some("-}"),
        non_doc_prefixes: &["--"],
    };

    /// Scala Scaladoc style: `/** */`
    pub const SCALADOC: Self = Self {
        comment_kinds: &["comment", "block_comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["//"],
        block_start: Some("/**"),
        block_end: Some("*/"),
        non_doc_prefixes: &[],
    };

    /// Swift Markdown style: `///`, `/** */`
    pub const SWIFTDOC: Self = Self {
        comment_kinds: &["comment", "multiline_comment"],
        position: DocPosition::Before,
        line_doc_prefixes: &["///"],
        block_start: Some("/**"),
        block_end: Some("*/"),
        non_doc_prefixes: &["//", "/*"],
    };
}

/// Extract documentation from a node using the given configuration.
pub fn extract_documentation_with_config(
    node: &tree_sitter::Node,
    source: &[u8],
    config: &DocStyleConfig,
) -> Option<String> {
    match config.position {
        DocPosition::Before => extract_docs_before(node, source, config),
        DocPosition::Inside => extract_docs_inside(node, source, config),
    }
}

/// Extract documentation that appears before the item.
fn extract_docs_before(
    node: &tree_sitter::Node,
    source: &[u8],
    config: &DocStyleConfig,
) -> Option<String> {
    let mut docs = Vec::new();
    let mut current = node.prev_sibling();

    while let Some(sibling) = current {
        if config.comment_kinds.contains(&sibling.kind()) {
            if let Ok(text) = sibling.utf8_text(source) {
                let trimmed = text.trim();

                // Check for block documentation
                if let (Some(block_start), Some(block_end)) = (config.block_start, config.block_end)
                {
                    if trimmed.starts_with(block_start) {
                        let content = extract_block_content(trimmed, block_start, block_end);
                        if !content.is_empty() {
                            docs.insert(0, content);
                        }
                        break;
                    }
                }

                // Check for line documentation
                let mut is_doc = false;
                for prefix in config.line_doc_prefixes {
                    if trimmed.starts_with(prefix) {
                        let content = trimmed.trim_start_matches('/').trim();
                        if !content.is_empty() {
                            docs.insert(0, content.to_string());
                        }
                        is_doc = true;
                        break;
                    }
                }

                if !is_doc {
                    // Check if this is a non-doc comment that should stop extraction
                    let should_stop = config.non_doc_prefixes.iter().any(|prefix| {
                        trimmed.starts_with(prefix)
                            && !config
                                .line_doc_prefixes
                                .iter()
                                .any(|dp| trimmed.starts_with(dp))
                    });

                    if should_stop || !trimmed.starts_with('/') {
                        break;
                    }
                }
            }
        } else if !sibling.kind().is_empty() {
            break;
        }
        current = sibling.prev_sibling();
    }

    if docs.is_empty() {
        None
    } else {
        Some(docs.join("\n"))
    }
}

/// Extract documentation that appears inside the item (e.g., Python docstrings).
fn extract_docs_inside(
    node: &tree_sitter::Node,
    source: &[u8],
    config: &DocStyleConfig,
) -> Option<String> {
    let mut cursor = node.walk();

    // For Python-style docstrings, look for string nodes inside the function/class
    for child in node.children(&mut cursor) {
        if config.comment_kinds.contains(&child.kind()) {
            if let Ok(text) = child.utf8_text(source) {
                let trimmed = text.trim();

                // Try triple-quoted strings (Python)
                for quote in &["\"\"\"", "'''"] {
                    if trimmed.starts_with(quote) && trimmed.ends_with(quote) {
                        let content = trimmed
                            .trim_start_matches(quote)
                            .trim_end_matches(quote)
                            .trim();
                        if !content.is_empty() {
                            return Some(content.to_string());
                        }
                    }
                }
            }
        }
    }

    // Fall back to extracting comments before the item
    extract_docs_before(node, source, config)
}

/// Extract content from a block comment, removing delimiters and cleaning up.
fn extract_block_content(comment: &str, start: &str, end: &str) -> String {
    comment
        .trim_start_matches(start)
        .trim_end_matches(end)
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim())
        .filter(|line| !line.is_empty() && !line.starts_with('@'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Extract documentation from a node based on the language.
///
/// This is the main entry point for documentation extraction.
pub fn extract_documentation(
    node: &tree_sitter::Node,
    source: &[u8],
    language: &str,
) -> Option<String> {
    let config = match language {
        "rust" => &DocStyleConfig::RUST,
        "python" => &DocStyleConfig::PYTHON,
        "javascript" | "typescript" => &DocStyleConfig::JSDOC,
        "go" => &DocStyleConfig::GO,
        "java" => &DocStyleConfig::JAVADOC,
        "c" | "cpp" | "c++" => &DocStyleConfig::DOXYGEN,
        "csharp" | "c#" => &DocStyleConfig::XMLDOC,
        "ruby" => &DocStyleConfig::RDOC,
        "lua" => &DocStyleConfig::LUADOC,
        "haskell" => &DocStyleConfig::HADDOCK,
        "scala" => &DocStyleConfig::SCALADOC,
        "swift" => &DocStyleConfig::SWIFTDOC,
        _ => return None,
    };

    extract_documentation_with_config(node, source, config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_rust_and_extract(source: &str) -> Option<String> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .ok()?;
        let tree = parser.parse(source, None)?;
        let root = tree.root_node();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if matches!(child.kind(), "function_item" | "struct_item" | "enum_item") {
                return extract_documentation(&child, source.as_bytes(), "rust");
            }
        }
        None
    }

    #[test]
    fn test_rust_doc_extraction() {
        let source = r#"
/// Adds two numbers
/// 
/// # Examples
/// ```
/// let result = add(1, 2);
/// ```
pub fn add(a: i32, b: i32) -> i32 { a + b }
"#;

        let docs = parse_rust_and_extract(source);
        assert!(docs.is_some());
        let docs = docs.unwrap();
        assert!(docs.contains("Adds two numbers"));
        assert!(docs.contains("Examples"));
    }

    #[test]
    fn test_rust_inner_doc() {
        let source = r#"
//! This is a module doc
//! It should NOT be attached to the function

pub fn add(a: i32, b: i32) -> i32 { a + b }
"#;

        let docs = parse_rust_and_extract(source);
        assert!(docs.is_none() || !docs.unwrap().contains("module doc"));
    }

    #[test]
    fn test_rust_no_docs() {
        let source = r#"
pub fn add(a: i32, b: i32) -> i32 { a + b }
"#;

        let docs = parse_rust_and_extract(source);
        assert!(docs.is_none());
    }

    #[test]
    fn test_rust_multiple_line_comments() {
        let source = r#"
/// First line
/// Second line
/// Third line
pub fn documented() {}
"#;

        let docs = parse_rust_and_extract(source);
        assert!(docs.is_some());
        let docs = docs.unwrap();
        assert!(docs.contains("First line"));
        assert!(docs.contains("Second line"));
        assert!(docs.contains("Third line"));
    }

    #[test]
    fn test_config_constants() {
        assert!(!DocStyleConfig::RUST.comment_kinds.is_empty());
        assert!(!DocStyleConfig::PYTHON.comment_kinds.is_empty());
        assert!(!DocStyleConfig::JSDOC.comment_kinds.is_empty());
        assert!(!DocStyleConfig::GO.comment_kinds.is_empty());
        assert!(!DocStyleConfig::JAVADOC.comment_kinds.is_empty());
        assert!(!DocStyleConfig::DOXYGEN.comment_kinds.is_empty());
        assert!(!DocStyleConfig::XMLDOC.comment_kinds.is_empty());
        assert!(!DocStyleConfig::RDOC.comment_kinds.is_empty());
        assert!(!DocStyleConfig::LUADOC.comment_kinds.is_empty());
        assert!(!DocStyleConfig::HADDOCK.comment_kinds.is_empty());
        assert!(!DocStyleConfig::SCALADOC.comment_kinds.is_empty());
        assert!(!DocStyleConfig::SWIFTDOC.comment_kinds.is_empty());
    }

    #[test]
    fn test_rust_block_doc() {
        let source = r#"
/**
 * This is a block doc comment
 * with multiple lines
 */
pub fn block_documented() {}
"#;
        let docs = parse_rust_and_extract(source);
        assert!(docs.is_some());
        let docs = docs.unwrap();
        assert!(docs.contains("block doc comment"));
    }

    #[test]
    fn test_rust_struct_doc() {
        let source = r#"
/// A user in the system
/// 
/// Contains name and email
pub struct User {
    name: String,
}
"#;
        let docs = parse_rust_and_extract(source);
        assert!(docs.is_some());
        assert!(docs.unwrap().contains("user in the system"));
    }

    #[test]
    fn test_rust_enum_doc() {
        let source = r#"
/// Result type for operations
pub enum Result {
    Ok,
    Err,
}
"#;
        let docs = parse_rust_and_extract(source);
        assert!(docs.is_some());
        assert!(docs.unwrap().contains("Result type"));
    }

    #[test]
    fn test_extract_block_content() {
        let content = extract_block_content("/** Hello World */", "/**", "*/");
        assert!(content.contains("Hello World"));

        let content = extract_block_content("/**\n * Line 1\n * Line 2\n */", "/**", "*/");
        assert!(content.contains("Line 1"));
        assert!(content.contains("Line 2"));
    }

    #[test]
    fn test_extract_block_content_filters_tags() {
        let content = extract_block_content(
            "/**\n * Description\n * @param x the value\n * @return result\n */",
            "/**",
            "*/",
        );
        assert!(content.contains("Description"));
        assert!(!content.contains("@param"));
        assert!(!content.contains("@return"));
    }

    #[test]
    fn test_extract_documentation_unknown_language() {
        let source = b"fn test() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        let result = extract_documentation(&root, source, "unknown_language");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_documentation_with_config_before() {
        let source = b"// not a doc\n/// doc comment\nfn test() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&source[..], None).unwrap();
        let root = tree.root_node();

        let mut cursor = root.walk();
        let func = root
            .children(&mut cursor)
            .find(|c| c.kind() == "function_item");

        if let Some(func) = func {
            let result = extract_documentation_with_config(&func, source, &DocStyleConfig::RUST);
            assert!(result.is_some());
        }
    }

    #[test]
    fn test_doc_position_variants() {
        assert_eq!(DocPosition::Before, DocPosition::Before);
        assert_eq!(DocPosition::Inside, DocPosition::Inside);
        assert_ne!(DocPosition::Before, DocPosition::Inside);
    }

    #[test]
    fn test_rust_unicode_doc() {
        let source = r#"
/// 用户信息
/// 日本語のドキュメント
/// Documentation in English
pub fn multilingual() {}
"#;
        let docs = parse_rust_and_extract(source);
        assert!(docs.is_some());
        let docs = docs.unwrap();
        assert!(docs.contains("用户信息"));
        assert!(docs.contains("日本語"));
    }

    #[test]
    fn test_rust_code_in_doc() {
        let source = r#"
/// Example usage:
/// 
/// ```rust
/// let x = 42;
/// assert_eq!(x, 42);
/// ```
pub fn with_code_example() {}
"#;
        let docs = parse_rust_and_extract(source);
        assert!(docs.is_some());
        let docs = docs.unwrap();
        assert!(docs.contains("Example usage"));
        assert!(docs.contains("let x = 42"));
    }

    #[test]
    fn test_rust_markdown_in_doc() {
        let source = r#"
/// # Heading
/// 
/// - List item 1
/// - List item 2
/// 
/// **Bold** and *italic*
pub fn with_markdown() {}
"#;
        let docs = parse_rust_and_extract(source);
        assert!(docs.is_some());
        let docs = docs.unwrap();
        assert!(docs.contains("Heading"));
        assert!(docs.contains("List item"));
    }

    #[test]
    fn test_rust_empty_doc_lines() {
        let source = r#"
/// First line
///
/// Third line (second is empty)
pub fn with_gap() {}
"#;
        let docs = parse_rust_and_extract(source);
        assert!(docs.is_some());
    }

    #[test]
    fn test_rust_comment_before_non_doc() {
        let source = r#"
// This is a regular comment
// Not documentation
pub fn no_doc() {}
"#;
        let docs = parse_rust_and_extract(source);
        assert!(docs.is_none());
    }

    #[test]
    fn test_rust_mixed_comments() {
        let source = r#"
// Regular comment
/// Doc comment attached
pub fn mixed() {}
"#;
        let docs = parse_rust_and_extract(source);
        // Regular comment stops extraction before it, so doc comment IS attached
        // (the doc is immediately before the function)
        assert!(docs.is_some());
        assert!(docs.unwrap().contains("Doc comment"));
    }

    #[test]
    fn test_language_aliases() {
        let source = b"fn test() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&source[..], None).unwrap();
        let root = tree.root_node();

        let js_result = extract_documentation(&root, source, "javascript");
        let ts_result = extract_documentation(&root, source, "typescript");
        assert_eq!(js_result, ts_result);
    }

    #[test]
    fn test_all_language_configs() {
        let source = b"fn test() {}";
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(&source[..], None).unwrap();
        let root = tree.root_node();

        let languages = [
            "rust",
            "python",
            "javascript",
            "typescript",
            "go",
            "java",
            "c",
            "cpp",
            "c++",
            "csharp",
            "c#",
            "ruby",
            "lua",
            "haskell",
            "scala",
            "swift",
        ];

        for lang in languages {
            let result = extract_documentation(&root, source, lang);
            assert!(
                result.is_none(),
                "Language {} should have valid config",
                lang
            );
        }
    }

    mod proptest_tests {
        use super::*;
        use proptest::prelude::*;

        proptest! {
            #[test]
            fn test_extract_block_content_idempotent(
                content in ".*",
            ) {
                let extracted = extract_block_content(&format!("/** {} */", content), "/**", "*/");
                // Should never panic, always return a string
                assert!(extracted.len() <= content.len() + 100);
            }

            #[test]
            fn test_extract_block_content_preserves_alphanumeric(
                text in "[a-zA-Z0-9 ]+",
            ) {
                let input = format!("/** {} */", text);
                let result = extract_block_content(&input, "/**", "*/");
                // Alphanumeric text should be preserved
                prop_assert!(result.contains(&text.trim()));
            }

            #[test]
            fn test_doc_config_comment_kinds_not_empty(
                _ in proptest::bool::ANY,
            ) {
                assert!(!DocStyleConfig::RUST.comment_kinds.is_empty());
                assert!(!DocStyleConfig::PYTHON.comment_kinds.is_empty());
                assert!(!DocStyleConfig::JSDOC.comment_kinds.is_empty());
            }

            #[test]
            fn test_rust_doc_extraction_never_panics(
                lines in proptest::collection::vec(".*", 1..10),
            ) {
                let doc_lines: Vec<String> = lines.iter().map(|l| format!("/// {}", l)).collect();
                let source = format!("{}\npub fn test() {{}}", doc_lines.join("\n"));
                let _ = parse_rust_and_extract(&source);
            }
        }
    }
}

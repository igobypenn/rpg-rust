use std::path::Path;

use crate::parser::docs::extract_documentation;
use crate::parser::helpers::TsNodeExt;
use crate::parser::{DefinitionInfo, ImportInfo};

pub fn extract_include(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
    if node.kind() != "preproc_include" {
        return None;
    }

    let path_node = node.child_by_field_name("path")?;
    let path_text = path_node.text(source);

    let (module_path, is_system) = if path_text.starts_with('"') {
        (path_text.trim_matches('"').to_string(), false)
    } else if path_text.starts_with('<') {
        (
            path_text
                .trim_start_matches('<')
                .trim_end_matches('>')
                .to_string(),
            true,
        )
    } else {
        (path_text.to_string(), false)
    };

    let mut import = ImportInfo::new(&module_path);
    import.location = Some(node.to_location(file));
    import.imported_names = vec![module_path.clone()];
    import.is_glob = true;

    import
        .metadata
        .insert("system".to_string(), serde_json::Value::Bool(is_system));

    Some(import)
}

pub fn extract_function(
    node: &tree_sitter::Node,
    source: &[u8],
    file: &Path,
    name: &str,
    language: &str,
) -> Option<DefinitionInfo> {
    if node.kind() != "function_definition" {
        return None;
    }

    let decl = node.child_by_field_name("declarator")?;

    let mut def = DefinitionInfo::new("fn", name);
    def.location = Some(node.to_location(file));
    if let Some(doc) = extract_documentation(node, source, language) {
        def.doc = Some(doc);
    }

    if let Some(type_node) = node.child_by_field_name("type") {
        let return_type = type_node.text(source);
        let params_text = extract_params_text(&decl, source);
        def.signature = Some(format!("{} {}{}", return_type, name, params_text));
    }

    let body = node.child_by_field_name("body");
    def.metadata.insert(
        "has_body".to_string(),
        serde_json::Value::Bool(body.is_some()),
    );

    def.is_public = true;

    Some(def)
}

pub fn extract_params_text(node: &tree_sitter::Node, source: &[u8]) -> String {
    if node.kind() == "function_declarator" {
        if let Some(params) = node.child_by_field_name("parameters") {
            return params.text(source).to_string();
        }
    }
    String::new()
}

pub fn find_function_declarator<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
    if node.kind() == "function_declarator" {
        return Some(*node);
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            if let Some(found) = find_function_declarator(&child) {
                return Some(found);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tree_sitter::Parser;

    fn parse_c(source: &str) -> (tree_sitter::Tree, Vec<u8>) {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        (tree, source.as_bytes().to_vec())
    }

    fn find_first_node<'a>(
        tree: &'a tree_sitter::Tree,
        _source: &[u8],
        kind: &str,
    ) -> Option<tree_sitter::Node<'a>> {
        let mut cursor = tree.root_node().walk();
        loop {
            if cursor.node().kind() == kind {
                return Some(cursor.node());
            }
            if cursor.goto_first_child() {
                continue;
            }
            if cursor.goto_next_sibling() {
                continue;
            }
            loop {
                if !cursor.goto_parent() {
                    return None;
                }
                if cursor.goto_next_sibling() {
                    break;
                }
            }
        }
    }

    #[test]
    fn test_extract_include_system() {
        let source = r#"#include <stdio.h>"#;
        let (tree, bytes) = parse_c(source);
        let node = find_first_node(&tree, &bytes, "preproc_include").unwrap();
        let result = extract_include(&node, &bytes, Path::new("test.c"));
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.module_path, "stdio.h");
    }

    #[test]
    fn test_extract_include_local() {
        let source = r#"#include "myheader.h""#;
        let (tree, bytes) = parse_c(source);
        let node = find_first_node(&tree, &bytes, "preproc_include").unwrap();
        let result = extract_include(&node, &bytes, Path::new("test.c"));
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.module_path, "myheader.h");
    }

    #[test]
    fn test_extract_function() {
        let source = r#"int add(int a, int b) { return a + b; }"#;
        let (tree, bytes) = parse_c(source);
        let node = find_first_node(&tree, &bytes, "function_definition").unwrap();
        let result = extract_function(&node, &bytes, Path::new("test.c"), "add", "c");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.name, "add");
    }

    #[test]
    fn test_extract_params_text() {
        let source = r#"int foo(int a, int b) { return a; }"#;
        let (tree, bytes) = parse_c(source);
        let fn_node = find_first_node(&tree, &bytes, "function_definition").unwrap();
        let declarator = fn_node.child_by_field_name("declarator").unwrap();
        let text = extract_params_text(&declarator, &bytes);
        assert!(text.contains("int a"));
        assert!(text.contains("int b"));
    }

    #[test]
    fn test_find_function_declarator() {
        let source = r#"int main() { return 0; }"#;
        let (tree, bytes) = parse_c(source);
        let fn_node = find_first_node(&tree, &bytes, "function_definition").unwrap();
        let declarator = find_function_declarator(&fn_node);
        assert!(declarator.is_some());
    }
}

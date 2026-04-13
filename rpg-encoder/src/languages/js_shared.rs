use std::collections::HashSet;
use std::path::Path;

use crate::parser::{
    base::collect_types, docs::extract_documentation, helpers::TsNodeExt, CallInfo, CallKind,
    DefinitionInfo, ImportInfo, ParseResult, TypeRefInfo,
};

pub fn extract_import(
    node: &tree_sitter::Node,
    source: &[u8],
    file: &Path,
    _language: &str,
) -> Option<ImportInfo> {
    match node.kind() {
        "import_statement" => {
            let source_node = node.child_by_field_name("source")?;
            let module_path = source_node
                .text(source)
                .trim_matches('"')
                .trim_matches('\'')
                .to_string();

            let mut import = ImportInfo::new(&module_path);
            import.location = Some(node.to_location(file));

            let mut names = Vec::new();
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "import_clause" {
                    let mut clause_cursor = child.walk();
                    for clause_child in child.children(&mut clause_cursor) {
                        match clause_child.kind() {
                            "identifier" => {
                                names.push(clause_child.text(source).to_string());
                            }
                            "named_imports" => {
                                let mut spec_cursor = clause_child.walk();
                                for spec in clause_child.children(&mut spec_cursor) {
                                    if spec.kind() == "import_specifier" {
                                        if let Some(name) = spec.child_by_field_name("name") {
                                            let name_text = name.text(source);
                                            if let Some(alias) = spec.child_by_field_name("alias") {
                                                names.push(format!(
                                                    "{} as {}",
                                                    name_text,
                                                    alias.text(source)
                                                ));
                                            } else {
                                                names.push(name_text.to_string());
                                            }
                                        }
                                    }
                                }
                            }
                            "namespace_import" => {
                                if let Some(name) = clause_child.child_by_field_name("name") {
                                    names.push(format!("* as {}", name.text(source)));
                                    import.is_glob = true;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }

            import.imported_names = names;
            Some(import)
        }
        "export_statement" => extract_reexport(node, source, file),
        _ => None,
    }
}

pub fn extract_reexport(
    node: &tree_sitter::Node,
    source: &[u8],
    file: &Path,
) -> Option<ImportInfo> {
    let source_node = node.child_by_field_name("source")?;
    let module_path = source_node
        .text(source)
        .trim_matches('"')
        .trim_matches('\'')
        .to_string();

    let mut import = ImportInfo::new(&module_path);
    import.location = Some(node.to_location(file));
    import.is_glob = true;

    Some(import)
}

pub fn extract_require_call(
    node: &tree_sitter::Node,
    source: &[u8],
    file: &Path,
) -> Option<ImportInfo> {
    if node.kind() != "call_expression" {
        return None;
    }

    let func = node.child_by_field_name("function")?;
    if func.kind() != "identifier" || func.text(source) != "require" {
        return None;
    }

    let args = node.child_by_field_name("arguments")?;
    let mut cursor = args.walk();
    let first_arg = args.children(&mut cursor).find(|c| c.is_named())?;

    if first_arg.kind() == "string" || first_arg.kind() == "template_string" {
        let arg_text = first_arg.text(source);
        let module_path = arg_text.trim_matches('"').trim_matches('\'').to_string();

        let mut import = ImportInfo::new(&module_path);
        import.location = Some(node.to_location(file));

        let parent = node.parent();
        if let Some(p) = parent {
            if p.kind() == "variable_declarator" {
                if let Some(name) = p.child_by_field_name("name") {
                    import.imported_names = vec![name.text(source).to_string()];
                }
            }
        }

        return Some(import);
    }

    None
}

pub fn extract_function(
    node: &tree_sitter::Node,
    source: &[u8],
    file: &Path,
    language: &str,
) -> Option<DefinitionInfo> {
    if node.kind() != "function_declaration" {
        return None;
    }

    let name = node.child_by_field_name("name").map(|n| n.text(source))?;

    let mut def = DefinitionInfo::new("function", name);
    def.location = Some(node.to_location(file));
    def.is_public = true;

    if let Some(params) = node.child_by_field_name("parameters") {
        let params_text = params.text(source);
        def.signature = Some(format!("{}{}", name, params_text));
    }

    if let Some(doc) = extract_documentation(node, source, language) {
        def.doc = Some(doc);
    }

    Some(def)
}

pub fn extract_type_refs(
    node: &tree_sitter::Node,
    source: &[u8],
    fn_name: &str,
    result: &mut ParseResult,
) {
    let mut seen = HashSet::new();
    let mut types = Vec::new();

    if let Some(params) = node.child_by_field_name("parameters") {
        let mut cursor = params.walk();
        for param in params.children(&mut cursor) {
            if param.kind() == "required_parameter"
                || param.kind() == "optional_parameter"
                || param.kind() == "rest_parameter"
            {
                if let Some(pattern) = param.child_by_field_name("pattern") {
                    if let Some(type_node) = pattern.child_by_field_name("type") {
                        seen.clear();
                        types.clear();
                        collect_types(
                            &type_node,
                            source,
                            &mut seen,
                            &mut types,
                            crate::languages::builtins::typescript::is_builtin,
                            &["type_identifier"],
                            &[],
                        );
                        for type_name in &types {
                            result
                                .type_refs
                                .push(TypeRefInfo::param(fn_name, type_name.clone()));
                        }
                    }
                }
                if let Some(type_node) = param.child_by_field_name("type") {
                    seen.clear();
                    types.clear();
                    collect_types(
                        &type_node,
                        source,
                        &mut seen,
                        &mut types,
                        crate::languages::builtins::typescript::is_builtin,
                        &["type_identifier"],
                        &[],
                    );
                    for type_name in &types {
                        result
                            .type_refs
                            .push(TypeRefInfo::param(fn_name, type_name.clone()));
                    }
                }
            }
        }
    }

    if let Some(return_type) = node.child_by_field_name("return_type") {
        seen.clear();
        types.clear();
        collect_types(
            &return_type,
            source,
            &mut seen,
            &mut types,
            crate::languages::builtins::typescript::is_builtin,
            &["type_identifier"],
            &[],
        );
        for type_name in &types {
            result
                .type_refs
                .push(TypeRefInfo::ret(fn_name, type_name.clone()));
        }
    }
}

pub fn extract_arrow_function(
    node: &tree_sitter::Node,
    source: &[u8],
    file: &Path,
) -> Option<DefinitionInfo> {
    if node.kind() != "arrow_function" {
        return None;
    }

    let parent = node.parent()?;
    let name = match parent.kind() {
        "variable_declarator" => parent.child_by_field_name("name").map(|n| n.text(source))?,
        "assignment_expression" => parent.child_by_field_name("left").map(|n| n.text(source))?,
        _ => return None,
    };

    let mut def = DefinitionInfo::new("arrow", name);
    def.location = Some(node.to_location(file));
    def.is_public = true;

    if let Some(params) = node.child_by_field_name("parameters") {
        let params_text = params.text(source);
        def.signature = Some(format!("{}{}", name, params_text));
    }

    Some(def)
}

pub fn extract_class(
    node: &tree_sitter::Node,
    source: &[u8],
    file: &Path,
    language: &str,
) -> Option<DefinitionInfo> {
    if node.kind() != "class_declaration" {
        return None;
    }

    let name = node.child_by_field_name("name").map(|n| n.text(source))?;

    let mut def = DefinitionInfo::new("class", name);
    def.location = Some(node.to_location(file));
    def.is_public = true;

    if let Some(parent) = node.child_by_field_name("parent_class") {
        let parent_name = parent.text(source);
        def.metadata.insert(
            "extends".to_string(),
            serde_json::Value::String(parent_name.to_string()),
        );
    }

    let body = node.child_by_field_name("body");
    if let Some(body) = body {
        let methods = extract_class_methods(&body, source);
        if !methods.is_empty() {
            def.metadata.insert(
                "methods".to_string(),
                serde_json::Value::Array(
                    methods.into_iter().map(serde_json::Value::String).collect(),
                ),
            );
        }
    }

    if let Some(doc) = extract_documentation(node, source, language) {
        def.doc = Some(doc);
    }

    Some(def)
}

pub fn extract_class_methods(body: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
    let mut methods = Vec::new();
    let mut cursor = body.walk();

    for child in body.children(&mut cursor) {
        if child.kind() == "method_definition" {
            if let Some(name) = child.child_by_field_name("name") {
                methods.push(name.text(source).to_string());
            }
        }
    }

    methods
}

pub fn extract_method(
    node: &tree_sitter::Node,
    source: &[u8],
    file: &Path,
    enclosing_class: &str,
    language: &str,
) -> Option<DefinitionInfo> {
    if node.kind() != "method_definition" {
        return None;
    }

    let name = node.child_by_field_name("name").map(|n| n.text(source))?;

    let mut def = DefinitionInfo::new("method", name);
    def.location = Some(node.to_location(file));
    def.parent = Some(enclosing_class.to_string());
    def.is_public = name != "constructor";

    if let Some(params) = node.child_by_field_name("parameters") {
        let params_text = params.text(source);
        def.signature = Some(format!("{}{}", name, params_text));
    }

    if let Some(doc) = extract_documentation(node, source, language) {
        def.doc = Some(doc);
    }

    Some(def)
}

pub fn extract_call(
    node: &tree_sitter::Node,
    source: &[u8],
    file: &Path,
    enclosing_fn: &str,
) -> Option<CallInfo> {
    if node.kind() != "call_expression" {
        return None;
    }

    let func = node.child_by_field_name("function")?;

    let location = node.to_location(file);

    let (callee, receiver, call_kind) = match func.kind() {
        "identifier" => {
            let name = func.text(source);
            if name == "require" {
                return None;
            }
            (name.to_string(), None, CallKind::Direct)
        }
        "member_expression" => {
            let obj = func.child_by_field_name("object");
            let prop = func.child_by_field_name("property");

            let receiver = obj.map(|o| o.text(source).to_string()).unwrap_or_default();
            let method = prop.map(|p| p.text(source).to_string()).unwrap_or_default();

            (method, Some(receiver), CallKind::Method)
        }
        _ => {
            return None;
        }
    };

    if callee.is_empty() {
        return None;
    }

    let call = CallInfo::new(enclosing_fn, callee)
        .with_kind(call_kind)
        .with_location(location);

    Some(if let Some(rec) = receiver {
        call.with_receiver(rec)
    } else {
        call
    })
}

pub fn for_each_named_child(
    node: tree_sitter::Node,
    source: &[u8],
    file: &Path,
    walk_fn: &mut dyn FnMut(&tree_sitter::Node, &[u8], &Path),
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.is_named() {
            walk_fn(&child, source, file);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tree_sitter::Parser;

    fn parse_js(source: &str) -> (tree_sitter::Tree, Vec<u8>) {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .unwrap();
        let tree = parser.parse(source, None).unwrap();
        (tree, source.as_bytes().to_vec())
    }

    fn parse_ts(source: &str) -> (tree_sitter::Tree, Vec<u8>) {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
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
    fn test_extract_import_es6() {
        let source = r#"import { foo } from './utils';"#;
        let (tree, bytes) = parse_js(source);
        let node = find_first_node(&tree, &bytes, "import_statement").unwrap();
        let result = extract_import(&node, &bytes, Path::new("test.js"), "javascript");
        assert!(result.is_some());
        let info = result.unwrap();
        assert!(info.module_path.contains("utils"));
    }

    #[test]
    fn test_extract_reexport() {
        let source = r#"export { foo } from './utils';"#;
        let (tree, bytes) = parse_js(source);
        let node = find_first_node(&tree, &bytes, "export_statement").unwrap();
        let result = extract_reexport(&node, &bytes, Path::new("test.js"));
        assert!(result.is_some());
    }

    #[test]
    fn test_extract_require_call() {
        let source = r#"const utils = require('./utils');"#;
        let (tree, bytes) = parse_js(source);
        let node = find_first_node(&tree, &bytes, "call_expression").unwrap();
        let result = extract_require_call(&node, &bytes, Path::new("test.js"));
        assert!(result.is_some());
        let info = result.unwrap();
        assert!(info.module_path.contains("utils"));
    }

    #[test]
    fn test_extract_function() {
        let source = r#"function myFunc(x) { return x; }"#;
        let (tree, bytes) = parse_js(source);
        let node = find_first_node(&tree, &bytes, "function_declaration").unwrap();
        let result = extract_function(&node, &bytes, Path::new("test.js"), "javascript");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.name, "myFunc");
    }

    #[test]
    fn test_extract_arrow_function() {
        let source = r#"const add = (a, b) => a + b;"#;
        let (tree, bytes) = parse_js(source);
        let node = find_first_node(&tree, &bytes, "arrow_function").unwrap();
        let result = extract_arrow_function(&node, &bytes, Path::new("test.js"));
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.name, "add");
    }

    #[test]
    fn test_extract_class() {
        let source = r#"class MyClass { constructor() {} }"#;
        let (tree, bytes) = parse_js(source);
        let node = find_first_node(&tree, &bytes, "class_declaration").unwrap();
        let result = extract_class(&node, &bytes, Path::new("test.js"), "javascript");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.name, "MyClass");
    }

    #[test]
    fn test_extract_class_methods() {
        let source = r#"class Foo { method1() {} method2() {} }"#;
        let (tree, bytes) = parse_js(source);
        let class_node = find_first_node(&tree, &bytes, "class_declaration").unwrap();
        let body = class_node.child_by_field_name("body").unwrap();
        let methods = extract_class_methods(&body, &bytes);
        assert!(methods.contains(&"method1".to_string()));
        assert!(methods.contains(&"method2".to_string()));
    }

    #[test]
    fn test_extract_method() {
        let source = r#"class Foo { myMethod(x) { return x; } }"#;
        let (tree, bytes) = parse_js(source);
        let node = find_first_node(&tree, &bytes, "method_definition").unwrap();
        let result = extract_method(&node, &bytes, Path::new("test.js"), "Foo", "javascript");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.name, "myMethod");
    }

    #[test]
    fn test_extract_call() {
        let source = r#"function test() { helper(); }"#;
        let (tree, bytes) = parse_js(source);
        let call_node = find_first_node(&tree, &bytes, "call_expression").unwrap();
        let result = extract_call(&call_node, &bytes, Path::new("test.js"), "test");
        assert!(result.is_some());
        let info = result.unwrap();
        assert_eq!(info.callee, "helper");
    }

    #[test]
    fn test_for_each_named_child() {
        let source = r#"function foo() { bar(); baz(); }"#;
        let (tree, bytes) = parse_js(source);
        let mut visited = Vec::new();
        for_each_named_child(
            tree.root_node(),
            &bytes,
            Path::new("test.js"),
            &mut |node, _src, _path| {
                visited.push(node.kind().to_string());
            },
        );
        assert!(!visited.is_empty());
    }

    #[test]
    fn test_extract_type_refs_typescript() {
        let source = r#"function process(config: Config): Result { return null; }"#;
        let (tree, bytes) = parse_ts(source);
        let fn_node = find_first_node(&tree, &bytes, "function_declaration").unwrap();
        let mut result = ParseResult::new(PathBuf::from("test.ts"));
        extract_type_refs(&fn_node, &bytes, "process", &mut result);
        assert!(
            !result.type_refs.is_empty(),
            "Should extract type references from TypeScript"
        );
    }
}

use std::path::Path;

use tree_sitter::Parser;

use crate::define_parser;
use crate::error::{Result, RpgError};
use crate::languages::ffi::FfiDetector;
use crate::languages::js_shared;
use crate::parser::helpers::TsNodeExt;
use crate::parser::{ParseResult, TreeSitterParser};

define_parser!(TypeScriptParser, "typescript", &["ts", "tsx"]);

const LANG: &str = "typescript";

impl TypeScriptParser {
    fn extract_decorators(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut decorators = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "decorator" {
                let text = child.text(source);
                let text = text.trim_start_matches('@').trim();
                decorators.push(text.to_string());
            }
        }

        decorators
    }

    fn extract_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<crate::parser::DefinitionInfo> {
        let mut def = js_shared::extract_function(node, source, file, LANG)?;

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let return_type = node
                .child_by_field_name("return_type")
                .map(|r| r.text(source))
                .unwrap_or_default();
            def.signature = Some(format!("{}{}{}", def.name, params_text, return_type));
        }

        let decorators = Self::extract_decorators(node, source);
        if !decorators.is_empty() {
            def.metadata.insert(
                "decorators".to_string(),
                serde_json::Value::Array(
                    decorators
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        Some(def)
    }

    fn extract_arrow_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<crate::parser::DefinitionInfo> {
        let mut def = js_shared::extract_arrow_function(node, source, file)?;

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let return_type = node
                .child_by_field_name("return_type")
                .map(|r| r.text(source))
                .unwrap_or_default();
            def.signature = Some(format!("{}{}{}", def.name, params_text, return_type));
        }

        Some(def)
    }

    fn extract_class(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<crate::parser::DefinitionInfo> {
        let mut def = js_shared::extract_class(node, source, file, LANG)?;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "implements_clause" {
                let mut impl_cursor = child.walk();
                let interfaces: Vec<String> = child
                    .children(&mut impl_cursor)
                    .filter(|c| c.kind() == "type_identifier" || c.kind() == "generic_type")
                    .map(|c| c.text(source).to_string())
                    .collect();
                if !interfaces.is_empty() {
                    def.metadata.insert(
                        "implements".to_string(),
                        serde_json::Value::Array(
                            interfaces
                                .into_iter()
                                .map(serde_json::Value::String)
                                .collect(),
                        ),
                    );
                }
            }
        }

        let body = node.child_by_field_name("body");
        if let Some(body) = body {
            let mut methods = js_shared::extract_class_methods(&body, source);
            let mut body_cursor = body.walk();
            for child in body.children(&mut body_cursor) {
                if child.kind() == "public_field_definition" {
                    if let Some(name) = child.child_by_field_name("name") {
                        let name_text = name.text(source).to_string();
                        if !methods.contains(&name_text) {
                            methods.push(name_text);
                        }
                    }
                }
            }
            if !methods.is_empty() {
                def.metadata.insert(
                    "methods".to_string(),
                    serde_json::Value::Array(
                        methods.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );
            }
        }

        let decorators = Self::extract_decorators(node, source);
        if !decorators.is_empty() {
            def.metadata.insert(
                "decorators".to_string(),
                serde_json::Value::Array(
                    decorators
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        Some(def)
    }

    fn extract_interface(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<crate::parser::DefinitionInfo> {
        if node.kind() != "interface_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = crate::parser::DefinitionInfo::new("interface", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "extends_clause" {
                let mut ext_cursor = child.walk();
                let parents: Vec<String> = child
                    .children(&mut ext_cursor)
                    .filter(|c| c.kind() == "type_identifier")
                    .map(|c| c.text(source).to_string())
                    .collect();
                if !parents.is_empty() {
                    def.metadata.insert(
                        "extends".to_string(),
                        serde_json::Value::Array(
                            parents.into_iter().map(serde_json::Value::String).collect(),
                        ),
                    );
                }
            }
        }

        if let Some(doc) = crate::parser::docs::extract_documentation(node, source, "typescript") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    crate::simple_definition_public!(
        extract_type_alias,
        "type_alias_declaration",
        "type",
        "typescript"
    );

    crate::simple_definition_public!(extract_enum, "enum_declaration", "enum", "typescript");

    fn extract_method(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_class: &str,
    ) -> Option<crate::parser::DefinitionInfo> {
        let mut def = js_shared::extract_method(node, source, file, enclosing_class, LANG)?;

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let return_type = node
                .child_by_field_name("return_type")
                .map(|r| r.text(source))
                .unwrap_or_default();
            def.signature = Some(format!("{}{}{}", def.name, params_text, return_type));
        }

        Some(def)
    }

    fn walk_and_extract(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &mut String,
        enclosing_class: &mut Option<String>,
        result: &mut ParseResult,
    ) {
        match node.kind() {
            "import_statement" | "export_statement" => {
                if let Some(import) = js_shared::extract_import(node, source, file, LANG) {
                    result.imports.push(import);
                }
            }
            "call_expression" => {
                if let Some(import) = js_shared::extract_require_call(node, source, file) {
                    result.imports.push(import);
                } else if let Some(call) = js_shared::extract_call(node, source, file, enclosing_fn)
                {
                    result.calls.push(call);
                }
            }
            "function_declaration" => {
                if let Some(def) = Self::extract_function(node, source, file) {
                    let fn_name = def.name.clone();
                    js_shared::extract_type_refs(node, source, &fn_name, result);
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    if let Some(body) = node.child_by_field_name("body") {
                        let mut walk = |child: &tree_sitter::Node, src: &[u8], f: &Path| {
                            Self::walk_and_extract(
                                child,
                                src,
                                f,
                                enclosing_fn,
                                enclosing_class,
                                result,
                            );
                        };
                        js_shared::for_each_named_child(body, source, file, &mut walk);
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "arrow_function" => {
                if let Some(def) = Self::extract_arrow_function(node, source, file) {
                    let fn_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    if let Some(body) = node.child_by_field_name("body") {
                        if body.kind() == "statement_block" {
                            let mut walk = |child: &tree_sitter::Node, src: &[u8], f: &Path| {
                                Self::walk_and_extract(
                                    child,
                                    src,
                                    f,
                                    enclosing_fn,
                                    enclosing_class,
                                    result,
                                );
                            };
                            js_shared::for_each_named_child(body, source, file, &mut walk);
                        }
                    }

                    *enclosing_fn = prev_fn;
                }
            }
            "class_declaration" => {
                if let Some(def) = Self::extract_class(node, source, file) {
                    let class_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_class = enclosing_class.clone();
                    *enclosing_class = Some(class_name);

                    if let Some(body) = node.child_by_field_name("body") {
                        let mut walk = |child: &tree_sitter::Node, src: &[u8], f: &Path| {
                            Self::walk_and_extract(
                                child,
                                src,
                                f,
                                enclosing_fn,
                                enclosing_class,
                                result,
                            );
                        };
                        js_shared::for_each_named_child(body, source, file, &mut walk);
                    }

                    *enclosing_class = prev_class;
                    return;
                }
            }
            "interface_declaration" => {
                if let Some(def) = Self::extract_interface(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "type_alias_declaration" => {
                if let Some(def) = Self::extract_type_alias(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "enum_declaration" => {
                if let Some(def) = Self::extract_enum(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "method_definition" => {
                if let Some(class) = enclosing_class.as_ref() {
                    if let Some(def) = Self::extract_method(node, source, file, class) {
                        let fn_name = def.name.clone();
                        result.definitions.push(def);

                        let prev_fn = enclosing_fn.clone();
                        *enclosing_fn = fn_name;

                        if let Some(body) = node.child_by_field_name("body") {
                            let mut walk = |child: &tree_sitter::Node, src: &[u8], f: &Path| {
                                Self::walk_and_extract(
                                    child,
                                    src,
                                    f,
                                    enclosing_fn,
                                    enclosing_class,
                                    result,
                                );
                            };
                            js_shared::for_each_named_child(body, source, file, &mut walk);
                        }

                        *enclosing_fn = prev_fn;
                        return;
                    }
                }
            }
            _ => {}
        }

        let mut walk = |child: &tree_sitter::Node, src: &[u8], f: &Path| {
            Self::walk_and_extract(child, src, f, enclosing_fn, enclosing_class, result);
        };
        js_shared::for_each_named_child(*node, source, file, &mut walk);
    }
}

impl TreeSitterParser for TypeScriptParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
            .map_err(|e| RpgError::tree_sitter_error(Path::new(""), e.to_string()))
    }

    fn parse_impl(source: &str, path: &Path, parser: &mut Parser) -> Result<ParseResult> {
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| RpgError::tree_sitter_error(path, "Failed to parse source"))?;

        let mut result = ParseResult::new(path.to_path_buf());
        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        let mut cursor = root.walk();
        let mut enclosing_fn = String::new();
        let mut enclosing_class: Option<String> = None;

        for child in root.children(&mut cursor) {
            if child.is_named() {
                Self::walk_and_extract(
                    &child,
                    source_bytes,
                    path,
                    &mut enclosing_fn,
                    &mut enclosing_class,
                    &mut result,
                );
            }
        }

        let ffi_bindings = FfiDetector::detect_node_native(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

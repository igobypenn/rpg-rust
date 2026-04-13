use std::path::Path;

use tree_sitter::Parser;

use crate::define_parser;
use crate::error::{Result, RpgError};
use crate::languages::ffi::FfiDetector;
use crate::languages::js_shared;
use crate::parser::{ParseResult, TreeSitterParser};

define_parser!(JavaScriptParser, "javascript", &["js", "mjs", "cjs"]);

const LANG: &str = "javascript";

impl JavaScriptParser {
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
                if let Some(def) = js_shared::extract_function(node, source, file, LANG) {
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
                if let Some(def) = js_shared::extract_arrow_function(node, source, file) {
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
                if let Some(def) = js_shared::extract_class(node, source, file, LANG) {
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
            "method_definition" => {
                if let Some(class) = enclosing_class.as_ref() {
                    if let Some(def) = js_shared::extract_method(node, source, file, class, LANG) {
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

impl TreeSitterParser for JavaScriptParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
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

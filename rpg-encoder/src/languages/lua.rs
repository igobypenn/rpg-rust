use std::path::Path;

use tree_sitter::Parser;

use crate::error::{Result, RpgError};
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::{CachedParser, TreeSitterParser},
    docs::extract_documentation,
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ImportInfo, LanguageParser, ParseResult,
};

pub struct LuaParser {
    cached: CachedParser,
}

impl LuaParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            cached: CachedParser::new::<Self>()?,
        })
    }

    fn extract_require(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        if node.kind() != "function_call" {
            return None;
        }

        let name = node.child_by_field_name("name")?;
        if name.text(source) != "require" {
            return None;
        }

        let args = node.child_by_field_name("arguments")?;
        let mut cursor = args.walk();
        let first_arg = args.children(&mut cursor).find(|c| {
            c.kind() == "string" || c.kind() == "string_literal" || c.kind() == "literal_string"
        })?;

        let arg_text = first_arg.text(source);
        let module_path = arg_text
            .trim_matches('"')
            .trim_matches('\'')
            .trim_matches('[')
            .trim_matches(']')
            .to_string();

        let mut import = ImportInfo::new(&module_path);
        import.location = Some(node.to_location(file));
        import.imported_names = vec![module_path.clone()];

        Some(import)
    }

    fn extract_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "function_declaration" && node.kind() != "function_definition" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("function", name);
        def.location = Some(node.to_location(file));
        def.is_public = !name.contains('_') || name.starts_with("M.") || name.starts_with("mod.");

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            def.signature = Some(format!("{}{}", name, params_text));
        }

        if let Some(doc) = extract_documentation(node, source, "lua") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_method(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "method_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let receiver = node
            .child_by_field_name("object")
            .map(|o| o.text(source).to_string());

        let mut def = DefinitionInfo::new("method", name);
        def.location = Some(node.to_location(file));

        if let Some(ref recv) = receiver {
            def.parent = Some(recv.clone());
        }

        def.is_public = !name.starts_with('_');

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            def.signature = Some(format!("{}{}", name, params_text));
        }

        if let Some(doc) = extract_documentation(node, source, "lua") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_local_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "local_function_definition" && node.kind() != "local_function_declaration"
        {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("local_function", name);
        def.location = Some(node.to_location(file));
        def.is_public = false;

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            def.signature = Some(format!("{}{}", name, params_text));
        }

        if let Some(doc) = extract_documentation(node, source, "lua") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_call(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &str,
    ) -> Option<CallInfo> {
        if node.kind() != "function_call" && node.kind() != "method_call" {
            return None;
        }

        let location = node.to_location(file);

        if node.kind() == "function_call" {
            let name = node.child_by_field_name("name")?;
            let name_text = name.text(source);

            if matches!(name_text, "require" | "print") {
                return None;
            }

            let call = CallInfo::new(enclosing_fn, name_text.to_string())
                .with_kind(CallKind::Direct)
                .with_location(location);

            return Some(call);
        }

        if node.kind() == "method_call" {
            let method = node.child_by_field_name("method")?;
            let obj = node.child_by_field_name("object");

            let method_name = method.text(source).to_string();
            let receiver = obj.map(|o| o.text(source).to_string()).unwrap_or_default();

            let call = CallInfo::new(enclosing_fn, method_name)
                .with_kind(CallKind::Method)
                .with_location(location);

            return Some(call.with_receiver(receiver));
        }

        None
    }

    fn walk_and_extract(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &mut String,
        result: &mut ParseResult,
    ) {
        match node.kind() {
            "function_call" => {
                if let Some(import) = Self::extract_require(node, source, file) {
                    result.imports.push(import);
                } else if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "method_call" => {
                if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "function_declaration" | "function_definition" => {
                if let Some(def) = Self::extract_function(node, source, file) {
                    let fn_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            if child.is_named() {
                                Self::walk_and_extract(&child, source, file, enclosing_fn, result);
                            }
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "method_declaration" => {
                if let Some(def) = Self::extract_method(node, source, file) {
                    let fn_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            if child.is_named() {
                                Self::walk_and_extract(&child, source, file, enclosing_fn, result);
                            }
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "local_function_definition" | "local_function_declaration" => {
                if let Some(def) = Self::extract_local_function(node, source, file) {
                    let fn_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            if child.is_named() {
                                Self::walk_and_extract(&child, source, file, enclosing_fn, result);
                            }
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                Self::walk_and_extract(&child, source, file, enclosing_fn, result);
            }
        }
    }
}

impl TreeSitterParser for LuaParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_lua::LANGUAGE.into())
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

        for child in root.children(&mut cursor) {
            if child.is_named() {
                Self::walk_and_extract(&child, source_bytes, path, &mut enclosing_fn, &mut result);
            }
        }

        let ffi_bindings = FfiDetector::detect_luajit_ffi(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

impl LanguageParser for LuaParser {
    fn language_name(&self) -> &str {
        "lua"
    }

    fn file_extensions(&self) -> &[&str] {
        &["lua"]
    }

    fn parse(&self, source: &str, path: &Path) -> Result<ParseResult> {
        self.cached.parse::<Self>(source, path)
    }
}

use std::path::Path;

use tree_sitter::Parser;

use crate::define_parser;
use crate::error::{Result, RpgError};
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::TreeSitterParser, docs::extract_documentation, helpers::TsNodeExt, CallInfo, CallKind,
    DefinitionInfo, ImportInfo, ParseResult,
};

define_parser!(SwiftParser, "swift", &["swift"]);

impl SwiftParser {
    fn extract_modifiers(node: &tree_sitter::Node, source: &[u8]) -> (bool, bool) {
        let mut is_public = true;
        let mut is_static = false;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifier" || child.kind() == "attribute" {
                let text = child.text(source);
                match text {
                    "private" | "fileprivate" => is_public = false,
                    "internal" => is_public = false,
                    "public" | "open" => is_public = true,
                    "static" => is_static = true,
                    _ => {}
                }
            }
        }

        (is_public, is_static)
    }

    fn extract_import(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        if node.kind() != "import_declaration" {
            return None;
        }

        let mut module_name = String::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                module_name = child.text(source).to_string();
                break;
            }
        }

        if module_name.is_empty() {
            return None;
        }

        let mut import = ImportInfo::new(&module_name);
        import.location = Some(node.to_location(file));
        import.imported_names = vec![module_name.clone()];

        Some(import)
    }

    fn extract_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "function_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("func", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let return_type = node
                .child_by_field_name("return_type")
                .map(|r| r.text(source))
                .unwrap_or_default();
            def.signature = Some(format!("{}{}{}", name, params_text, return_type));
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "attribute" {
                let attr_text = child.text(source);
                if attr_text.contains("@objc") {
                    def.metadata
                        .insert("objc".to_string(), serde_json::Value::Bool(true));
                }
                if attr_text.contains("@_cdecl") || attr_text.contains("@_silgen_name") {
                    if let Some(c_name) = Self::extract_attr_string(attr_text) {
                        def.metadata
                            .insert("c_name".to_string(), serde_json::Value::String(c_name));
                    }
                }
            }
        }

        if let Some(doc) = extract_documentation(node, source, "swift") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_attr_string(attr: &str) -> Option<String> {
        let start = attr.find('"')?;
        let rest = &attr[start + 1..];
        let end = rest.find('"')?;
        Some(rest[..end].to_string())
    }

    fn extract_class(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "class_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("class", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "inheritance_clause" || child.kind() == "type_inheritance_clause" {
                let mut inh_cursor = child.walk();
                let parents: Vec<String> = child
                    .children(&mut inh_cursor)
                    .filter(|c| c.kind() == "simple_type_identifier" || c.kind() == "user_type")
                    .map(|c| c.text(source).to_string())
                    .collect();
                if !parents.is_empty() {
                    def.metadata.insert(
                        "inherits".to_string(),
                        serde_json::Value::Array(
                            parents.into_iter().map(serde_json::Value::String).collect(),
                        ),
                    );
                }
            }
        }

        if let Some(doc) = extract_documentation(node, source, "swift") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_struct(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "struct_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("struct", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if let Some(doc) = extract_documentation(node, source, "swift") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_protocol(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "protocol_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("protocol", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if let Some(doc) = extract_documentation(node, source, "swift") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_enum(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "enum_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("enum", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if let Some(doc) = extract_documentation(node, source, "swift") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_property(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "property_declaration" && node.kind() != "variable_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("property", name);
        def.location = Some(node.to_location(file));
        def.parent = Some(enclosing.to_string());
        def.is_public = is_public;

        if let Some(type_node) = node.child_by_field_name("type") {
            let type_text = type_node.text(source);
            def.signature = Some(format!("{}: {}", name, type_text));
        }

        Some(def)
    }

    fn extract_call(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &str,
    ) -> Option<CallInfo> {
        if node.kind() != "call_expression" {
            return None;
        }

        let func = node.child_by_field_name("called_expression")?;

        let location = node.to_location(file);

        let (callee, receiver, call_kind) = match func.kind() {
            "simple_identifier" | "identifier" => {
                let name = func.text(source);
                (name.to_string(), None, CallKind::Direct)
            }
            "member_expression" | "navigation_expression" => {
                let obj = func.child_by_field_name("object");
                let member = func.child_by_field_name("member");

                let receiver = obj.map(|o| o.text(source).to_string()).unwrap_or_default();
                let method = member
                    .map(|m| m.text(source).to_string())
                    .unwrap_or_default();

                (method, Some(receiver), CallKind::Method)
            }
            "constructor_expression" => {
                if let Some(type_node) = func.child_by_field_name("type") {
                    let name = type_node.text(source);
                    (name.to_string(), None, CallKind::Constructor)
                } else {
                    return None;
                }
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

    fn walk_and_extract(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &mut String,
        enclosing_type: &mut Option<String>,
        result: &mut ParseResult,
    ) {
        match node.kind() {
            "import_declaration" => {
                if let Some(import) = Self::extract_import(node, source, file) {
                    result.imports.push(import);
                }
            }
            "function_declaration" => {
                if let Some(def) = Self::extract_function(node, source, file) {
                    let fn_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            if child.is_named() {
                                Self::walk_and_extract(
                                    &child,
                                    source,
                                    file,
                                    enclosing_fn,
                                    enclosing_type,
                                    result,
                                );
                            }
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "class_declaration" => {
                if let Some(def) = Self::extract_class(node, source, file) {
                    let class_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_type = enclosing_type.clone();
                    *enclosing_type = Some(class_name);

                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            if child.is_named() {
                                Self::walk_and_extract(
                                    &child,
                                    source,
                                    file,
                                    enclosing_fn,
                                    enclosing_type,
                                    result,
                                );
                            }
                        }
                    }

                    *enclosing_type = prev_type;
                    return;
                }
            }
            "struct_declaration" => {
                if let Some(def) = Self::extract_struct(node, source, file) {
                    let struct_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_type = enclosing_type.clone();
                    *enclosing_type = Some(struct_name);

                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            if child.is_named() {
                                Self::walk_and_extract(
                                    &child,
                                    source,
                                    file,
                                    enclosing_fn,
                                    enclosing_type,
                                    result,
                                );
                            }
                        }
                    }

                    *enclosing_type = prev_type;
                    return;
                }
            }
            "protocol_declaration" => {
                if let Some(def) = Self::extract_protocol(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "enum_declaration" => {
                if let Some(def) = Self::extract_enum(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "property_declaration" | "variable_declaration" => {
                let enclosing = enclosing_type
                    .as_ref()
                    .map(|s| s.as_str())
                    .unwrap_or_else(|| enclosing_fn.as_str());
                if let Some(def) = Self::extract_property(node, source, file, enclosing) {
                    result.definitions.push(def);
                }
            }
            "call_expression" => {
                if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                Self::walk_and_extract(&child, source, file, enclosing_fn, enclosing_type, result);
            }
        }
    }
}

impl TreeSitterParser for SwiftParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_swift::LANGUAGE.into())
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
        let mut enclosing_type: Option<String> = None;

        for child in root.children(&mut cursor) {
            if child.is_named() {
                Self::walk_and_extract(
                    &child,
                    source_bytes,
                    path,
                    &mut enclosing_fn,
                    &mut enclosing_type,
                    &mut result,
                );
            }
        }

        let ffi_bindings = FfiDetector::detect_swift_ffi(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

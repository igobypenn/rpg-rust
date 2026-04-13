use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Parser;

use crate::define_parser;
use crate::error::{Result, RpgError};
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::{collect_types_with_scoped, TreeSitterParser},
    docs::extract_documentation,
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ImportInfo, ParseResult, TypeRefInfo, TypeRefKind,
};

define_parser!(RustParser, "rust", &["rs"]);

impl RustParser {
    fn extract_use_declaration(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<ImportInfo> {
        if node.kind() != "use_declaration" {
            return None;
        }

        fn extract_use_path(node: &tree_sitter::Node, source: &[u8]) -> String {
            match node.kind() {
                "scoped_identifier" | "identifier" | "crate" | "super" | "self" => {
                    node.text(source).to_string()
                }
                "use_clause" | "scoped_use_list" | "use_list" => node
                    .children(&mut node.walk())
                    .filter(|c| c.is_named())
                    .map(|c| extract_use_path(&c, source))
                    .collect::<Vec<_>>()
                    .join("::"),
                "use_wildcard" => "*".to_string(),
                "use_as_clause" => {
                    let name = node
                        .child_by_field_name("name")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    let alias = node
                        .child_by_field_name("alias")
                        .map(|n| n.text(source).to_string())
                        .unwrap_or_default();
                    format!("{} as {}", name, alias)
                }
                "use_bounded_list" => node
                    .children(&mut node.walk())
                    .filter(|c| c.is_named() && c.kind() != "use_bounded_list")
                    .map(|c| extract_use_path(&c, source))
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => node.text(source).to_string(),
            }
        }

        let mut cursor = node.walk();
        if let Some(child) = node.children(&mut cursor).find(|c| c.is_named()) {
            let path_text = extract_use_path(&child, source);

            let mut import = ImportInfo::new(&path_text);
            import.location = Some(node.to_location(file));

            if path_text.contains('*') {
                import.is_glob = true;
            } else {
                let clean_path = path_text
                    .replace("::", ".")
                    .replace("crate::", "")
                    .replace("std::", "std.");
                let parts: Vec<&str> = clean_path.split('.').collect();
                if let Some(last) = parts.last() {
                    if !last.is_empty() {
                        import.imported_names = vec![last.to_string()];
                    }
                }
            }

            return Some(import);
        }

        None
    }

    fn extract_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "function_item" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("fn", &name);
        def.location = Some(node.to_location(file));

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let return_type = node
                .child_by_field_name("return_type")
                .map(|r| r.text(source).to_string())
                .unwrap_or_default();
            def.signature = Some(format!("{}{}{}", name, params_text, return_type));
        }

        def.is_public = node
            .child_by_field_name("visibility")
            .map(|v| v.text(source).contains("pub"))
            .unwrap_or(false);

        if let Some(doc) = extract_documentation(node, source, "rust") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_struct(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "struct_item" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("struct", name);
        def.location = Some(node.to_location(file));
        def.is_public = node
            .child_by_field_name("visibility")
            .map(|v| v.text(source).contains("pub"))
            .unwrap_or(false);

        if let Some(doc) = extract_documentation(node, source, "rust") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_enum(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "enum_item" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("enum", name);
        def.location = Some(node.to_location(file));
        def.is_public = node
            .child_by_field_name("visibility")
            .map(|v| v.text(source).contains("pub"))
            .unwrap_or(false);

        if let Some(doc) = extract_documentation(node, source, "rust") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_trait(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "trait_item" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("trait", name);
        def.location = Some(node.to_location(file));
        def.is_public = node
            .child_by_field_name("visibility")
            .map(|v| v.text(source).contains("pub"))
            .unwrap_or(false);

        if let Some(doc) = extract_documentation(node, source, "rust") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_impl(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "impl_item" {
            return None;
        }

        let name = node
            .child_by_field_name("type")
            .map(|n| n.text(source).to_string())?;

        let trait_name = node
            .child_by_field_name("trait")
            .map(|n| n.text(source).to_string());

        let kind = if trait_name.is_some() {
            "impl_trait"
        } else {
            "impl"
        };

        let mut def = DefinitionInfo::new(kind, name);
        def.location = Some(node.to_location(file));

        if let Some(trait_name) = trait_name {
            def.metadata.insert("trait".to_string(), trait_name.into());
        }

        Some(def)
    }

    fn extract_const(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "const_item" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("const", name);
        def.location = Some(node.to_location(file));
        def.is_public = node
            .child_by_field_name("visibility")
            .map(|v| v.text(source).contains("pub"))
            .unwrap_or(false);

        if let Some(doc) = extract_documentation(node, source, "rust") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_static(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "static_item" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("static", name);
        def.location = Some(node.to_location(file));
        def.is_public = node
            .child_by_field_name("visibility")
            .map(|v| v.text(source).contains("pub"))
            .unwrap_or(false);

        if let Some(doc) = extract_documentation(node, source, "rust") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_mod(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<DefinitionInfo> {
        if node.kind() != "mod_item" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("mod", name);
        def.location = Some(node.to_location(file));
        def.is_public = node
            .child_by_field_name("visibility")
            .map(|v| v.text(source).contains("pub"))
            .unwrap_or(false);

        if let Some(doc) = extract_documentation(node, source, "rust") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_type_alias(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "type_item" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("type", name);
        def.location = Some(node.to_location(file));
        def.is_public = node
            .child_by_field_name("visibility")
            .map(|v| v.text(source).contains("pub"))
            .unwrap_or(false);

        if let Some(doc) = extract_documentation(node, source, "rust") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_call_expression(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &str,
        result: &mut ParseResult,
    ) {
        if node.kind() != "call_expression" {
            return;
        }

        let Some(func) = node.child_by_field_name("function") else {
            return;
        };

        let location = node.to_location(file);

        let (callee, receiver, call_kind) = match func.kind() {
            "identifier" => {
                let name = func.text(source).to_string();
                (name, None, CallKind::Direct)
            }
            "scoped_identifier" => {
                let text = func.text(source).to_string();
                let parts: Vec<&str> = text.split("::").collect();
                if parts.len() >= 2 {
                    let method = parts.last().unwrap_or(&"").to_string();
                    let type_part = parts[..parts.len() - 1].join("::");
                    if ["self", "Self"].contains(&type_part.as_str()) {
                        (method, Some("self".to_string()), CallKind::Method)
                    } else if method
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false)
                    {
                        (text.clone(), Some(type_part), CallKind::Constructor)
                    } else {
                        (text.clone(), Some(type_part), CallKind::Associated)
                    }
                } else {
                    (text, None, CallKind::Direct)
                }
            }
            "field_expression" => {
                let obj = func.child_by_field_name("object");
                let field = func.child_by_field_name("field");

                let receiver = obj.map(|o| o.text(source).to_string()).unwrap_or_default();
                let method = field
                    .map(|f| f.text(source).to_string())
                    .unwrap_or_default();

                (method, Some(receiver), CallKind::Method)
            }
            "macro_invocation" => {
                let name = func.text(source);
                (
                    name.trim_end_matches('!').to_string(),
                    None,
                    CallKind::Macro,
                )
            }
            _ => {
                return;
            }
        };

        if callee.is_empty() {
            return;
        }

        let call = CallInfo::new(enclosing_fn, callee)
            .with_kind(call_kind)
            .with_location(location);

        let call = if let Some(rec) = receiver {
            call.with_receiver(rec)
        } else {
            call
        };

        result.calls.push(call);
    }

    fn extract_type_refs_from_function(
        node: &tree_sitter::Node,
        source: &[u8],
        fn_name: &str,
        result: &mut ParseResult,
    ) {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut types: Vec<String> = Vec::new();

        if let Some(params) = node.child_by_field_name("parameters") {
            let mut cursor = params.walk();
            for param in params.children(&mut cursor) {
                if param.kind() == "parameter" {
                    if let Some(type_node) = param.child_by_field_name("type") {
                        seen.clear();
                        types.clear();
                        collect_types_with_scoped(
                            &type_node,
                            source,
                            &mut seen,
                            &mut types,
                            crate::languages::builtins::rust::is_builtin,
                            &["type_identifier"],
                            &["scoped_type_identifier"],
                            &["generic_type"],
                        );
                        for type_name in &types {
                            result
                                .type_refs
                                .push(TypeRefInfo::param(fn_name, type_name));
                        }
                    }
                }
            }
        }

        if let Some(return_type) = node.child_by_field_name("return_type") {
            seen.clear();
            types.clear();
            collect_types_with_scoped(
                &return_type,
                source,
                &mut seen,
                &mut types,
                crate::languages::builtins::rust::is_builtin,
                &["type_identifier"],
                &["scoped_type_identifier"],
                &["generic_type"],
            );
            for type_name in &types {
                result.type_refs.push(TypeRefInfo::ret(fn_name, type_name));
            }
        }
    }

    fn extract_type_refs_from_struct(
        node: &tree_sitter::Node,
        source: &[u8],
        struct_name: &str,
        result: &mut ParseResult,
    ) {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut types: Vec<String> = Vec::new();

        if let Some(body) = node.child_by_field_name("body") {
            let mut cursor = body.walk();
            for field in body.children(&mut cursor) {
                if field.kind() == "field_declaration" {
                    if let Some(type_node) = field.child_by_field_name("type") {
                        seen.clear();
                        types.clear();
                        collect_types_with_scoped(
                            &type_node,
                            source,
                            &mut seen,
                            &mut types,
                            crate::languages::builtins::rust::is_builtin,
                            &["type_identifier"],
                            &["scoped_type_identifier"],
                            &["generic_type"],
                        );
                        for type_name in &types {
                            result
                                .type_refs
                                .push(TypeRefInfo::field(struct_name, type_name));
                        }
                    }
                }
            }
        }
    }

    fn walk_and_extract(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &mut String,
        result: &mut ParseResult,
    ) {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut types: Vec<String> = Vec::new();

        match node.kind() {
            "use_declaration" => {
                if let Some(import) = Self::extract_use_declaration(node, source, file) {
                    result.imports.push(import);
                }
            }
            "function_item" => {
                if let Some(def) = Self::extract_function(node, source, file) {
                    let fn_name = def.name.clone();
                    Self::extract_type_refs_from_function(node, source, &fn_name, result);
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.is_named() {
                            Self::walk_and_extract(&child, source, file, enclosing_fn, result);
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "struct_item" => {
                if let Some(def) = Self::extract_struct(node, source, file) {
                    let struct_name = def.name.clone();
                    Self::extract_type_refs_from_struct(node, source, &struct_name, result);
                    result.definitions.push(def);
                }
            }
            "enum_item" => {
                if let Some(def) = Self::extract_enum(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "trait_item" => {
                if let Some(def) = Self::extract_trait(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "impl_item" => {
                if let Some(def) = Self::extract_impl(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "const_item" => {
                if let Some(def) = Self::extract_const(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "static_item" => {
                if let Some(def) = Self::extract_static(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "mod_item" => {
                if let Some(def) = Self::extract_mod(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "type_item" => {
                if let Some(def) = Self::extract_type_alias(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "call_expression" => {
                Self::extract_call_expression(node, source, file, enclosing_fn, result);
            }
            "let_declaration" => {
                if let Some(type_node) = node.child_by_field_name("type") {
                    seen.clear();
                    types.clear();
                    collect_types_with_scoped(
                        &type_node,
                        source,
                        &mut seen,
                        &mut types,
                        crate::languages::builtins::rust::is_builtin,
                        &["type_identifier"],
                        &["scoped_type_identifier"],
                        &["generic_type"],
                    );
                    for type_name in &types {
                        result.type_refs.push(
                            TypeRefInfo::new(enclosing_fn.as_str(), type_name)
                                .with_kind(TypeRefKind::Local),
                        );
                    }
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

impl TreeSitterParser for RustParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
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

        let mut ffi_bindings = FfiDetector::detect_no_mangle(source, path);
        ffi_bindings.extend(FfiDetector::detect_extern_blocks(source, path, &["C"]));
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

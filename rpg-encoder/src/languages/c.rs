use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Parser;

use super::c_shared;
use crate::define_parser;
use crate::error::{Result, RpgError};
use crate::languages::builtins;
use crate::parser::{
    base::{collect_types, TreeSitterParser},
    docs::extract_documentation,
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ParseResult, TypeRefInfo,
};

define_parser!(CParser, "c", &["c", "h"]);

impl CParser {
    fn extract_declaration(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "declaration" {
            return None;
        }

        if let Some(decl) = node.child_by_field_name("declarator") {
            if Self::is_function_pointer(&decl) {
                return Self::extract_function_pointer(node, &decl, source, file);
            }
        }

        if let Some(type_node) = node.child_by_field_name("type") {
            let type_text = type_node.text(source);

            if let Some(decl) = node.child_by_field_name("declarator") {
                let name = Self::extract_decl_name(&decl, source);

                if type_text == "typedef" {
                    return None;
                }

                let mut def = DefinitionInfo::new("var", name.clone());
                def.location = Some(node.to_location(file));
                if let Some(doc) = extract_documentation(node, source, "c") {
                    def.doc = Some(doc);
                }
                def.signature = Some(format!("{} {}", type_text, name));
                def.is_public = true;

                return Some(def);
            }
        }

        None
    }

    fn extract_typedef(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "type_definition" {
            return None;
        }

        let name = node
            .child_by_field_name("declarator")
            .map(|d| Self::extract_decl_name(&d, source))?;

        let mut def = DefinitionInfo::new("typedef", &name);
        def.location = Some(node.to_location(file));
        if let Some(doc) = extract_documentation(node, source, "c") {
            def.doc = Some(doc);
        }

        if let Some(type_node) = node.child_by_field_name("type") {
            let alias_type = type_node.text(source);
            def.signature = Some(format!("typedef {} {}", alias_type, name));
        }

        def.is_public = true;

        Some(def)
    }

    fn extract_struct(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "struct_specifier" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("struct", name);
        def.location = Some(node.to_location(file));
        if let Some(doc) = extract_documentation(node, source, "c") {
            def.doc = Some(doc);
        }
        def.is_public = true;

        let fields = Self::extract_struct_fields(node, source);
        if !fields.is_empty() {
            def.metadata.insert(
                "fields".to_string(),
                serde_json::Value::Array(
                    fields.into_iter().map(serde_json::Value::String).collect(),
                ),
            );
        }

        Some(def)
    }

    crate::simple_definition_public!(extract_union, "union_specifier", "union", "c");

    crate::simple_definition_public!(extract_enum, "enum_specifier", "enum", "c");

    fn extract_fn_name_from_declarator(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "identifier" => Some(node.text(source).to_string()),
            "parenthesized_declarator" | "pointer_declarator" => {
                let inner = node.child(0)?;
                Self::extract_fn_name_from_declarator(&inner, source)
            }
            "function_declarator" => {
                let decl = node.child_by_field_name("declarator")?;
                Self::extract_fn_name_from_declarator(&decl, source)
            }
            _ => None,
        }
    }

    fn extract_decl_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        match node.kind() {
            "identifier" => node.text(source).to_string(),
            "pointer_declarator" | "array_declarator" | "parenthesized_declarator" => node
                .children(&mut node.walk())
                .find(|c| c.is_named())
                .map(|c| Self::extract_decl_name(&c, source))
                .unwrap_or_default(),
            "function_declarator" => node
                .child_by_field_name("declarator")
                .map(|d| Self::extract_decl_name(&d, source))
                .unwrap_or_default(),
            _ => node.text(source).to_string(),
        }
    }

    fn is_function_pointer(node: &tree_sitter::Node) -> bool {
        if node.kind() == "pointer_declarator" {
            if let Some(inner) = node.child(0) {
                if inner.kind() == "parenthesized_declarator" {
                    if let Some(func_decl) = inner.child(0) {
                        return func_decl.kind() == "function_declarator";
                    }
                }
                if inner.kind() == "function_declarator" {
                    return true;
                }
            }
        }
        false
    }

    fn extract_function_pointer(
        node: &tree_sitter::Node,
        decl: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        let name = Self::extract_fn_ptr_name(decl, source)?;

        let mut def = DefinitionInfo::new("fn_ptr", &name);
        def.location = Some(node.to_location(file));
        if let Some(doc) = extract_documentation(node, source, "c") {
            def.doc = Some(doc);
        }

        if let Some(type_node) = node.child_by_field_name("type") {
            let return_type = type_node.text(source);
            let sig = Self::extract_fn_ptr_signature(decl, source);
            def.signature = Some(format!("{} (*{}){}", return_type, name, sig));
        }

        def.is_public = true;

        Some(def)
    }

    fn extract_fn_ptr_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        if node.kind() == "pointer_declarator" {
            if let Some(inner) = node.child(0) {
                if inner.kind() == "parenthesized_declarator" {
                    if let Some(func_decl) = inner.child(0) {
                        if func_decl.kind() == "function_declarator" {
                            if let Some(decl) = func_decl.child_by_field_name("declarator") {
                                return Some(decl.text(source).to_string());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn extract_fn_ptr_signature(node: &tree_sitter::Node, source: &[u8]) -> String {
        if node.kind() == "pointer_declarator" {
            if let Some(inner) = node.child(0) {
                if inner.kind() == "parenthesized_declarator" {
                    if let Some(func_decl) = inner.child(0) {
                        if func_decl.kind() == "function_declarator" {
                            if let Some(params) = func_decl.child_by_field_name("parameters") {
                                return params.text(source).to_string();
                            }
                        }
                    }
                }
            }
        }
        String::new()
    }

    fn extract_struct_fields(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut fields = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "field_declaration_list" {
                let mut field_cursor = child.walk();
                for field in child.children(&mut field_cursor) {
                    if field.kind() == "field_declaration" {
                        if let Some(decl) = field.child_by_field_name("declarator") {
                            fields.push(Self::extract_decl_name(&decl, source));
                        }
                    }
                }
            }
        }

        fields
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

        let func = node.child_by_field_name("function")?;

        let location = node.to_location(file);

        let callee = match func.kind() {
            "identifier" => func.text(source).to_string(),
            _ => return None,
        };

        if callee.is_empty() {
            return None;
        }

        Some(
            CallInfo::new(enclosing_fn, callee)
                .with_kind(CallKind::Direct)
                .with_location(location),
        )
    }

    fn extract_type_refs_from_func(
        node: &tree_sitter::Node,
        source: &[u8],
        fn_name: &str,
        result: &mut ParseResult,
    ) {
        let mut seen = HashSet::new();
        let mut types = Vec::new();

        if let Some(type_node) = node.child_by_field_name("type") {
            seen.clear();
            types.clear();
            collect_types(
                &type_node,
                source,
                &mut seen,
                &mut types,
                builtins::c::is_builtin,
                &["type_identifier"],
                &["pointer_type", "array_type"],
            );
            for type_name in &types {
                result
                    .type_refs
                    .push(TypeRefInfo::ret(fn_name, type_name.clone()));
            }
        }

        if let Some(decl) = node.child_by_field_name("declarator") {
            if let Some(func_decl) = c_shared::find_function_declarator(&decl) {
                if let Some(params) = func_decl.child_by_field_name("parameters") {
                    let mut cursor = params.walk();
                    for param in params.children(&mut cursor) {
                        if param.kind() == "parameter_declaration" {
                            if let Some(type_node) = param.child_by_field_name("type") {
                                seen.clear();
                                types.clear();
                                collect_types(
                                    &type_node,
                                    source,
                                    &mut seen,
                                    &mut types,
                                    builtins::c::is_builtin,
                                    &["type_identifier"],
                                    &["pointer_type", "array_type"],
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
        match node.kind() {
            "preproc_include" => {
                if let Some(import) = c_shared::extract_include(node, source, file) {
                    result.imports.push(import);
                }
            }
            "function_definition" => {
                if let Some(def) = node
                    .child_by_field_name("declarator")
                    .and_then(|decl| Self::extract_fn_name_from_declarator(&decl, source))
                    .and_then(|name| c_shared::extract_function(node, source, file, &name, "c"))
                {
                    let fn_name = def.name.clone();
                    Self::extract_type_refs_from_func(node, source, &fn_name, result);
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
            "declaration" => {
                if let Some(def) = Self::extract_declaration(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "type_definition" => {
                if let Some(def) = Self::extract_typedef(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "struct_specifier" => {
                if let Some(def) = Self::extract_struct(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "union_specifier" => {
                if let Some(def) = Self::extract_union(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "enum_specifier" => {
                if let Some(def) = Self::extract_enum(node, source, file) {
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
                Self::walk_and_extract(&child, source, file, enclosing_fn, result);
            }
        }
    }
}

impl TreeSitterParser for CParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_c::LANGUAGE.into())
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

        Ok(result)
    }
}

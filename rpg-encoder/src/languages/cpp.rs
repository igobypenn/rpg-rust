use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Parser;

use super::c_shared;
use crate::define_parser;
use crate::error::{Result, RpgError};
use crate::languages::builtins;
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::{collect_types, TreeSitterParser},
    docs::extract_documentation,
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ParseResult, TypeRefInfo, TypeRefKind,
};

define_parser!(
    CppParser,
    "cpp",
    &["cpp", "cc", "cxx", "hpp", "hh", "hxx", "h"]
);

impl CppParser {
    fn extract_fn_name_from_declarator(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        match node.kind() {
            "identifier" => Some(node.text(source).to_string()),
            "qualified_identifier" => {
                let name = node.child_by_field_name("name")?;
                Some(name.text(source).to_string())
            }
            "reference_declarator" | "pointer_declarator" | "parenthesized_declarator" => {
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

    fn extract_class(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "class_specifier" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("class", name);
        def.location = Some(node.to_location(file));
        if let Some(doc) = extract_documentation(node, source, "cpp") {
            def.doc = Some(doc);
        }
        def.is_public = true;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "base_class_clause" {
                let bases = Self::extract_base_classes(&child, source);
                if !bases.is_empty() {
                    def.metadata.insert(
                        "bases".to_string(),
                        serde_json::Value::Array(
                            bases.into_iter().map(serde_json::Value::String).collect(),
                        ),
                    );
                }
            }
        }

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
        if let Some(doc) = extract_documentation(node, source, "cpp") {
            def.doc = Some(doc);
        }
        def.is_public = true;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "base_class_clause" {
                let bases = Self::extract_base_classes(&child, source);
                if !bases.is_empty() {
                    def.metadata.insert(
                        "bases".to_string(),
                        serde_json::Value::Array(
                            bases.into_iter().map(serde_json::Value::String).collect(),
                        ),
                    );
                }
            }
        }

        Some(def)
    }

    fn extract_base_classes(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut bases = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_identifier" {
                bases.push(child.text(source).to_string());
            }
            if child.kind() == "qualified_identifier" {
                if let Some(name) = child.child_by_field_name("name") {
                    bases.push(name.text(source).to_string());
                }
            }
        }

        bases
    }

    crate::simple_definition_public!(
        extract_namespace,
        "namespace_definition",
        "namespace",
        "cpp"
    );

    fn extract_template(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "template_declaration" {
            return None;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "function_definition" => {
                    if let Some(mut def) = child
                        .child_by_field_name("declarator")
                        .and_then(|decl| Self::extract_fn_name_from_declarator(&decl, source))
                        .and_then(|name| {
                            c_shared::extract_function(&child, source, file, &name, "cpp")
                        })
                    {
                        def.kind = "template_fn".to_string();

                        if let Some(params) = node.child_by_field_name("parameters") {
                            let template_params = Self::extract_template_params(&params, source);
                            def.metadata.insert(
                                "template_params".to_string(),
                                serde_json::Value::Array(
                                    template_params
                                        .into_iter()
                                        .map(serde_json::Value::String)
                                        .collect(),
                                ),
                            );
                        }

                        return Some(def);
                    }
                }
                "class_specifier" => {
                    if let Some(mut def) = Self::extract_class(&child, source, file) {
                        def.kind = "template_class".to_string();

                        if let Some(params) = node.child_by_field_name("parameters") {
                            let template_params = Self::extract_template_params(&params, source);
                            def.metadata.insert(
                                "template_params".to_string(),
                                serde_json::Value::Array(
                                    template_params
                                        .into_iter()
                                        .map(serde_json::Value::String)
                                        .collect(),
                                ),
                            );
                        }

                        return Some(def);
                    }
                }
                "struct_specifier" => {
                    if let Some(mut def) = Self::extract_struct(&child, source, file) {
                        def.kind = "template_struct".to_string();

                        if let Some(params) = node.child_by_field_name("parameters") {
                            let template_params = Self::extract_template_params(&params, source);
                            def.metadata.insert(
                                "template_params".to_string(),
                                serde_json::Value::Array(
                                    template_params
                                        .into_iter()
                                        .map(serde_json::Value::String)
                                        .collect(),
                                ),
                            );
                        }

                        return Some(def);
                    }
                }
                _ => {}
            }
        }

        None
    }

    fn extract_template_params(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut params = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameter_declaration" {
                if let Some(name) = child.child_by_field_name("name") {
                    params.push(name.text(source).to_string());
                } else {
                    params.push(child.text(source).to_string());
                }
            }
            if child.kind() == "parameter_declaration" {
                if let Some(name) = child.child_by_field_name("declarator") {
                    params.push(name.text(source).to_string());
                }
            }
        }

        params
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

        let (callee, receiver, call_kind) = match func.kind() {
            "identifier" => {
                let name = func.text(source).to_string();
                (name, None, CallKind::Direct)
            }
            "qualified_identifier" => {
                let scope = func
                    .child_by_field_name("scope")
                    .map(|s| s.text(source).to_string());
                let name = func
                    .child_by_field_name("name")
                    .map(|n| n.text(source).to_string())?;

                if let Some(scope) = scope {
                    (name.clone(), Some(scope), CallKind::Method)
                } else {
                    (name, None, CallKind::Direct)
                }
            }
            "field_expression" => {
                let obj = func.child_by_field_name("argument");
                let field = func.child_by_field_name("field");

                let receiver = obj.map(|o| o.text(source).to_string()).unwrap_or_default();
                let method = field
                    .map(|f| f.text(source).to_string())
                    .unwrap_or_default();

                (method, Some(receiver), CallKind::Method)
            }
            "template_function" => {
                let name = func.text(source);
                let name = name.split('<').next().unwrap_or(name).to_string();
                (name, None, CallKind::Direct)
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
                builtins::cpp::is_builtin,
                &["type_identifier"],
                &[
                    "pointer_type",
                    "reference_type",
                    "array_type",
                    "template_type",
                    "generic_type",
                ],
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
                                    builtins::cpp::is_builtin,
                                    &["type_identifier"],
                                    &[
                                        "pointer_type",
                                        "reference_type",
                                        "array_type",
                                        "template_type",
                                        "generic_type",
                                    ],
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
        enclosing_ns: &mut Option<String>,
        result: &mut ParseResult,
    ) {
        match node.kind() {
            "preproc_include" => {
                if let Some(import) = c_shared::extract_include(node, source, file) {
                    result.imports.push(import);
                }
            }
            "function_definition" => {
                if let Some(mut def) = node
                    .child_by_field_name("declarator")
                    .and_then(|decl| Self::extract_fn_name_from_declarator(&decl, source))
                    .and_then(|name| c_shared::extract_function(node, source, file, &name, "cpp"))
                {
                    let fn_name = def.name.clone();
                    Self::extract_type_refs_from_func(node, source, &fn_name, result);

                    if let Some(ref ns) = *enclosing_ns {
                        def.parent = Some(ns.clone());
                    }

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
                                    enclosing_ns,
                                    result,
                                );
                            }
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "class_specifier" => {
                if let Some(mut def) = Self::extract_class(node, source, file) {
                    if let Some(ref ns) = *enclosing_ns {
                        def.parent = Some(ns.clone());
                    }
                    result.definitions.push(def);
                }
            }
            "struct_specifier" => {
                if let Some(mut def) = Self::extract_struct(node, source, file) {
                    if let Some(ref ns) = *enclosing_ns {
                        def.parent = Some(ns.clone());
                    }
                    result.definitions.push(def);
                }
            }
            "namespace_definition" => {
                if let Some(def) = Self::extract_namespace(node, source, file) {
                    let ns_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_ns = enclosing_ns.clone();
                    *enclosing_ns = Some(ns_name);

                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.is_named() {
                            Self::walk_and_extract(
                                &child,
                                source,
                                file,
                                enclosing_fn,
                                enclosing_ns,
                                result,
                            );
                        }
                    }

                    *enclosing_ns = prev_ns;
                    return;
                }
            }
            "template_declaration" => {
                if let Some(mut def) = Self::extract_template(node, source, file) {
                    if let Some(ref ns) = *enclosing_ns {
                        def.parent = Some(ns.clone());
                    }
                    result.definitions.push(def);
                }
            }
            "call_expression" => {
                if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "declaration" => {
                if let Some(type_node) = node.child_by_field_name("type") {
                    let mut seen = HashSet::new();
                    let mut types = Vec::new();
                    collect_types(
                        &type_node,
                        source,
                        &mut seen,
                        &mut types,
                        builtins::cpp::is_builtin,
                        &["type_identifier"],
                        &[
                            "pointer_type",
                            "reference_type",
                            "array_type",
                            "template_type",
                            "generic_type",
                        ],
                    );
                    for type_name in types {
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
                Self::walk_and_extract(&child, source, file, enclosing_fn, enclosing_ns, result);
            }
        }
    }
}

impl TreeSitterParser for CppParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
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
        let mut enclosing_ns: Option<String> = None;

        for child in root.children(&mut cursor) {
            if child.is_named() {
                Self::walk_and_extract(
                    &child,
                    source_bytes,
                    path,
                    &mut enclosing_fn,
                    &mut enclosing_ns,
                    &mut result,
                );
            }
        }

        let ffi_bindings = FfiDetector::detect_cpp_extern_c(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

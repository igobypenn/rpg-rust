use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Parser;

use crate::define_parser;
use crate::error::{Result, RpgError};
use crate::languages::builtins;
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::{collect_types, TreeSitterParser},
    docs::extract_documentation,
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ImportInfo, ParseResult, TypeRefInfo, TypeRefKind,
};

define_parser!(PythonParser, "python", &["py", "pyi"]);

impl PythonParser {
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

    fn extract_import(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        match node.kind() {
            "import_statement" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "dotted_name" || child.kind() == "aliased_import" {
                        if child.kind() == "aliased_import" {
                            let name = child
                                .child_by_field_name("name")
                                .map(|n| n.text(source).to_string())?;
                            let alias = child
                                .child_by_field_name("alias")
                                .map(|n| n.text(source).to_string());

                            let mut import = ImportInfo::new(&name);
                            import.location = Some(node.to_location(file));
                            if let Some(a) = alias {
                                import.imported_names = vec![format!("{} as {}", name, a)];
                            } else {
                                import.imported_names = vec![name];
                            }
                            return Some(import);
                        } else {
                            let name = child.text(source).to_string();
                            let mut import = ImportInfo::new(&name);
                            import.location = Some(node.to_location(file));
                            import.imported_names = vec![name.clone()];
                            return Some(import);
                        }
                    }
                }
            }
            "import_from_statement" => {
                let module = node
                    .child_by_field_name("module_name")
                    .map(|n| n.text(source).to_string())?;

                let mut import = ImportInfo::new(&module);
                import.location = Some(node.to_location(file));

                let mut cursor = node.walk();
                let mut names = Vec::new();

                for child in node.children(&mut cursor) {
                    match child.kind() {
                        "dotted_name" | "identifier" => {
                            let name = child.text(source).to_string();
                            if name != module {
                                names.push(name);
                            }
                        }
                        "aliased_import" => {
                            if let (Some(name), Some(alias)) = (
                                child.child_by_field_name("name"),
                                child.child_by_field_name("alias"),
                            ) {
                                names.push(format!(
                                    "{} as {}",
                                    name.text(source),
                                    alias.text(source)
                                ));
                            }
                        }
                        "wildcard_import" => {
                            import.is_glob = true;
                        }
                        _ => {}
                    }
                }

                import.imported_names = names;
                return Some(import);
            }
            _ => {}
        }
        None
    }

    fn extract_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "function_definition" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("def", &name);
        def.location = Some(node.to_location(file));

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let return_type = node
                .child_by_field_name("return_type")
                .map(|r| r.text(source).to_string())
                .unwrap_or_default();
            def.signature = Some(format!("{}{}{}", name, params_text, return_type));
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

        def.is_public = !name.starts_with('_');

        if let Some(doc) = extract_documentation(node, source, "python") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_class(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "class_definition" {
            return None;
        }

        let name = node
            .child_by_field_name("name")
            .map(|n| n.text(source).to_string())?;

        let mut def = DefinitionInfo::new("class", name.clone());
        def.location = Some(node.to_location(file));

        let mut arg_list = node.child_by_field_name("superclasses");
        if arg_list.is_none() {
            arg_list = node
                .children(&mut node.walk())
                .find(|c| c.kind() == "argument_list" || c.kind() == "parenthesized_expression");
        }

        if let Some(args) = arg_list {
            let mut cursor = args.walk();
            let bases: Vec<String> = args
                .children(&mut cursor)
                .filter(|c| c.is_named() && c.kind() != "(" && c.kind() != ")")
                .map(|c| c.text(source).to_string())
                .collect();

            if !bases.is_empty() {
                def.metadata.insert(
                    "bases".to_string(),
                    serde_json::Value::Array(
                        bases.into_iter().map(serde_json::Value::String).collect(),
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

        def.is_public = !name.starts_with('_');

        if let Some(doc) = extract_documentation(node, source, "python") {
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
        if node.kind() != "call" {
            return None;
        }

        let func = node.child_by_field_name("function")?;

        let location = node.to_location(file);

        let (callee, receiver, call_kind) = match func.kind() {
            "identifier" => {
                let name = func.text(source).to_string();
                (name, None, CallKind::Direct)
            }
            "attribute" => {
                let obj = func.child_by_field_name("object");
                let attr = func.child_by_field_name("attribute");

                let receiver = obj.map(|o| o.text(source).to_string()).unwrap_or_default();
                let method = attr.map(|a| a.text(source).to_string()).unwrap_or_default();

                if method.starts_with(|c: char| c.is_uppercase()) && !receiver.is_empty() {
                    let full_name = format!("{}.{}", receiver, method);
                    (full_name, Some(receiver), CallKind::Constructor)
                } else {
                    (method, Some(receiver), CallKind::Method)
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

    fn extract_type_annotations(
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
                if param.kind() == "identifier" || param.kind() == "typed_parameter" {
                    if let Some(type_node) = param.child_by_field_name("type") {
                        seen.clear();
                        types.clear();
                        collect_types(
                            &type_node,
                            source,
                            &mut seen,
                            &mut types,
                            builtins::python::is_builtin,
                            &["identifier", "type_identifier"],
                            &["subscript", "tuple", "list", "parenthesized_expression"],
                        );
                        for type_name in &types {
                            result
                                .type_refs
                                .push(TypeRefInfo::param(fn_name, type_name));
                        }
                    }
                }
                if param.kind() == "default_parameter" {
                    if let Some(inner) = param.child(0) {
                        if let Some(type_node) = inner.child_by_field_name("type") {
                            seen.clear();
                            types.clear();
                            collect_types(
                                &type_node,
                                source,
                                &mut seen,
                                &mut types,
                                builtins::python::is_builtin,
                                &["identifier", "type_identifier"],
                                &["subscript", "tuple", "list", "parenthesized_expression"],
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
        }

        if let Some(return_type) = node.child_by_field_name("return_type") {
            seen.clear();
            types.clear();
            collect_types(
                &return_type,
                source,
                &mut seen,
                &mut types,
                builtins::python::is_builtin,
                &["identifier", "type_identifier"],
                &["subscript", "tuple", "list", "parenthesized_expression"],
            );
            for type_name in &types {
                result.type_refs.push(TypeRefInfo::ret(fn_name, type_name));
            }
        }
    }

    fn walk_and_extract(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &mut String,
        enclosing_class: &mut Option<String>,
        result: &mut ParseResult,
    ) {
        let mut seen: HashSet<&str> = HashSet::new();
        let mut types: Vec<String> = Vec::new();

        match node.kind() {
            "import_statement" | "import_from_statement" => {
                if let Some(import) = Self::extract_import(node, source, file) {
                    result.imports.push(import);
                }
            }
            "function_definition" => {
                if let Some(mut def) = Self::extract_function(node, source, file) {
                    let fn_name = def.name.clone();
                    Self::extract_type_annotations(node, source, &fn_name, result);

                    if let Some(ref class) = *enclosing_class {
                        def.parent = Some(class.clone());
                        def.metadata
                            .insert("is_method".to_string(), serde_json::Value::Bool(true));
                    }

                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.is_named() && child.kind() != "block" {
                            Self::walk_and_extract(
                                &child,
                                source,
                                file,
                                enclosing_fn,
                                enclosing_class,
                                result,
                            );
                        }
                    }
                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            if child.is_named() {
                                Self::walk_and_extract(
                                    &child,
                                    source,
                                    file,
                                    enclosing_fn,
                                    enclosing_class,
                                    result,
                                );
                            }
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "class_definition" => {
                if let Some(def) = Self::extract_class(node, source, file) {
                    let class_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_class = enclosing_class.clone();
                    *enclosing_class = Some(class_name);

                    if let Some(body) = node.child_by_field_name("body") {
                        let mut cursor = body.walk();
                        for child in body.children(&mut cursor) {
                            if child.is_named() {
                                Self::walk_and_extract(
                                    &child,
                                    source,
                                    file,
                                    enclosing_fn,
                                    enclosing_class,
                                    result,
                                );
                            }
                        }
                    }

                    *enclosing_class = prev_class;
                    return;
                }
            }
            "call" => {
                if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "assignment" => {
                if let Some(type_node) = node.child_by_field_name("type") {
                    seen.clear();
                    types.clear();
                    collect_types(
                        &type_node,
                        source,
                        &mut seen,
                        &mut types,
                        builtins::python::is_builtin,
                        &["identifier", "type_identifier"],
                        &["subscript", "tuple", "list", "parenthesized_expression"],
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
                Self::walk_and_extract(&child, source, file, enclosing_fn, enclosing_class, result);
            }
        }
    }
}

impl TreeSitterParser for PythonParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
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

        let ffi_bindings = FfiDetector::detect_python_ctypes(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

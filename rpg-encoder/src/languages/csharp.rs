use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Parser;

use crate::define_parser;
use crate::error::{Result, RpgError};
use crate::languages::builtins;
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::{collect_types_with_scoped, TreeSitterParser},
    docs::extract_documentation,
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ImportInfo, ParseResult, TypeRefInfo, TypeRefKind,
};

define_parser!(CSharpParser, "csharp", &["cs"]);

impl CSharpParser {
    fn extract_modifiers(node: &tree_sitter::Node, source: &[u8]) -> (bool, bool) {
        let mut is_public = false;
        let mut is_static = false;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifier" {
                let text = child.text(source);
                match text {
                    "public" => is_public = true,
                    "static" => is_static = true,
                    _ => {}
                }
            }
        }

        (is_public, is_static)
    }

    fn extract_using(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        if node.kind() != "using_directive" {
            return None;
        }

        let mut cursor = node.walk();
        let mut module_path = String::new();
        let mut is_static = false;
        let mut alias: Option<String> = None;

        for child in node.children(&mut cursor) {
            match child.kind() {
                "identifier" | "qualified_name" | "name_equals" => {
                    module_path = child.text(source).to_string();
                    if module_path.contains('=') {
                        let parts: Vec<&str> = module_path.split('=').collect();
                        if parts.len() == 2 {
                            alias = Some(parts[0].trim().to_string());
                            module_path = parts[1].trim().to_string();
                        }
                    }
                }
                "static" => {
                    is_static = true;
                }
                _ => {}
            }
        }

        if module_path.is_empty() {
            return None;
        }

        let mut import = ImportInfo::new(&module_path);
        import.location = Some(node.to_location(file));
        import.is_glob = is_static;

        if let Some(alias_name) = alias {
            import
                .metadata
                .insert("alias".to_string(), serde_json::Value::String(alias_name));
        }

        let name = module_path.rsplit('.').next().unwrap_or(&module_path);
        import.imported_names = vec![name.to_string()];

        Some(import)
    }

    fn extract_namespace(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        if node.kind() != "namespace_declaration" {
            return None;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "qualified_name" {
                return Some(child.text(source).to_string());
            }
        }

        None
    }

    fn extract_class(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        namespace: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "class_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("class", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if !namespace.is_empty() {
            def.metadata.insert(
                "namespace".to_string(),
                serde_json::Value::String(namespace.to_string()),
            );
        }

        if let Some(base_list) = node.child_by_field_name("base_list") {
            let mut cursor = base_list.walk();
            let bases: Vec<String> = base_list
                .children(&mut cursor)
                .filter(|c| c.kind() == "identifier" || c.kind() == "qualified_name")
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

        if let Some(type_params) = Self::extract_type_params(node, source) {
            def.metadata.insert(
                "type_params".to_string(),
                serde_json::Value::Array(
                    type_params
                        .into_iter()
                        .map(serde_json::Value::String)
                        .collect(),
                ),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "csharp") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_interface(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        namespace: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "interface_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("interface", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if !namespace.is_empty() {
            def.metadata.insert(
                "namespace".to_string(),
                serde_json::Value::String(namespace.to_string()),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "csharp") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_struct(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        namespace: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "struct_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("struct", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if !namespace.is_empty() {
            def.metadata.insert(
                "namespace".to_string(),
                serde_json::Value::String(namespace.to_string()),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "csharp") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_enum(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        namespace: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "enum_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("enum", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if !namespace.is_empty() {
            def.metadata.insert(
                "namespace".to_string(),
                serde_json::Value::String(namespace.to_string()),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "csharp") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_record(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        namespace: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "record_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("record", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if !namespace.is_empty() {
            def.metadata.insert(
                "namespace".to_string(),
                serde_json::Value::String(namespace.to_string()),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "csharp") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_method(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_class: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "method_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, is_static) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("method", name);
        def.location = Some(node.to_location(file));
        def.parent = Some(enclosing_class.to_string());
        def.is_public = is_public;

        if let Some(return_type) = node.child_by_field_name("type") {
            let return_text = return_type.text(source);
            if let Some(params) = node.child_by_field_name("parameters") {
                let params_text = params.text(source);
                def.signature = Some(format!("{} {}{}", return_text, name, params_text));
            }
        }

        if is_static {
            def.metadata
                .insert("static".to_string(), serde_json::Value::Bool(true));
        }

        if let Some(doc) = extract_documentation(node, source, "csharp") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_property(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_class: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "property_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, is_static) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("property", name);
        def.location = Some(node.to_location(file));
        def.parent = Some(enclosing_class.to_string());
        def.is_public = is_public;

        if let Some(type_node) = node.child_by_field_name("type") {
            let type_text = type_node.text(source);
            def.signature = Some(format!("{} {}", type_text, &name));
        }

        if is_static {
            def.metadata
                .insert("static".to_string(), serde_json::Value::Bool(true));
        }

        if let Some(doc) = extract_documentation(node, source, "csharp") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_field(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_class: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "field_declaration" {
            return None;
        }

        let (is_public, is_static) = Self::extract_modifiers(node, source);

        if let Some(decl) = node.child_by_field_name("declarator") {
            if let Some(name_node) = decl.child_by_field_name("name") {
                let name = name_node.text(source);

                let mut def = DefinitionInfo::new("field", name);
                def.location = Some(node.to_location(file));
                def.parent = Some(enclosing_class.to_string());
                def.is_public = is_public;

                if let Some(type_node) = node.child_by_field_name("type") {
                    let type_text = type_node.text(source);
                    def.signature = Some(format!("{} {}", type_text, &name));
                }

                if is_static {
                    def.metadata
                        .insert("static".to_string(), serde_json::Value::Bool(true));
                }

                return Some(def);
            }
        }

        None
    }

    fn extract_call(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &str,
    ) -> Option<CallInfo> {
        if node.kind() != "invocation_expression" {
            return None;
        }

        let name = node
            .child_by_field_name("function")
            .map(|n| Self::extract_call_name(&n, source))?;

        let location = node.to_location(file);

        let call = CallInfo::new(enclosing_fn, &name)
            .with_kind(CallKind::Method)
            .with_location(location);

        Some(call)
    }

    fn extract_call_name(node: &tree_sitter::Node, source: &[u8]) -> String {
        let text = node.text(source);
        if let Some(last) = text.split('.').next_back() {
            last.to_string()
        } else {
            text.to_string()
        }
    }

    fn extract_type_params(node: &tree_sitter::Node, source: &[u8]) -> Option<Vec<String>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameter_list" {
                let mut tp_cursor = child.walk();
                let params: Vec<String> = child
                    .children(&mut tp_cursor)
                    .filter(|c| c.kind() == "type_parameter")
                    .map(|c| c.text(source).to_string())
                    .collect();
                if !params.is_empty() {
                    return Some(params);
                }
            }
        }
        None
    }

    fn extract_type_refs_from_method(
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
            collect_types_with_scoped(
                &type_node,
                source,
                &mut seen,
                &mut types,
                builtins::csharp::is_builtin,
                &["identifier", "type_identifier"],
                &["qualified_name"],
                &[
                    "generic_name",
                    "array_type",
                    "nullable_type",
                    "pointer_type",
                ],
            );
            for type_name in &types {
                result
                    .type_refs
                    .push(TypeRefInfo::ret(fn_name, type_name.clone()));
            }
        }

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
                            builtins::csharp::is_builtin,
                            &["identifier", "type_identifier"],
                            &["qualified_name"],
                            &[
                                "generic_name",
                                "array_type",
                                "nullable_type",
                                "pointer_type",
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

    fn walk_and_extract(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &mut String,
        enclosing_class: &mut Option<String>,
        namespace: &str,
        result: &mut ParseResult,
    ) {
        match node.kind() {
            "using_directive" => {
                if let Some(import) = Self::extract_using(node, source, file) {
                    result.imports.push(import);
                }
            }
            "namespace_declaration" => {
                let ns_name = Self::extract_namespace(node, source).unwrap_or_default();
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
                                &ns_name,
                                result,
                            );
                        }
                    }
                }
                return;
            }
            "class_declaration" => {
                if let Some(def) = Self::extract_class(node, source, file, namespace) {
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
                                    namespace,
                                    result,
                                );
                            }
                        }
                    }

                    *enclosing_class = prev_class;
                    return;
                }
            }
            "interface_declaration" => {
                if let Some(def) = Self::extract_interface(node, source, file, namespace) {
                    result.definitions.push(def);
                }
            }
            "struct_declaration" => {
                if let Some(def) = Self::extract_struct(node, source, file, namespace) {
                    result.definitions.push(def);
                }
            }
            "enum_declaration" => {
                if let Some(def) = Self::extract_enum(node, source, file, namespace) {
                    result.definitions.push(def);
                }
            }
            "record_declaration" => {
                if let Some(def) = Self::extract_record(node, source, file, namespace) {
                    result.definitions.push(def);
                }
            }
            "method_declaration" => {
                if let Some(class) = enclosing_class.as_ref() {
                    if let Some(def) = Self::extract_method(node, source, file, class) {
                        let fn_name = def.name.clone();
                        Self::extract_type_refs_from_method(node, source, &fn_name, result);
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
                                        enclosing_class,
                                        namespace,
                                        result,
                                    );
                                }
                            }
                        }

                        *enclosing_fn = prev_fn;
                        return;
                    }
                }
            }
            "property_declaration" => {
                if let Some(class) = enclosing_class.as_ref() {
                    if let Some(def) = Self::extract_property(node, source, file, class) {
                        result.definitions.push(def);
                    }
                }
            }
            "field_declaration" => {
                if let Some(class) = enclosing_class.as_ref() {
                    if let Some(def) = Self::extract_field(node, source, file, class) {
                        result.definitions.push(def);
                    }
                }
            }
            "invocation_expression" => {
                if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "local_declaration_statement" => {
                if let Some(var_decl) = node.child_by_field_name("declarator") {
                    if let Some(type_node) = var_decl.child_by_field_name("type") {
                        let mut seen = HashSet::new();
                        let mut types = Vec::new();
                        collect_types_with_scoped(
                            &type_node,
                            source,
                            &mut seen,
                            &mut types,
                            builtins::csharp::is_builtin,
                            &["identifier", "type_identifier"],
                            &["qualified_name"],
                            &[
                                "generic_name",
                                "array_type",
                                "nullable_type",
                                "pointer_type",
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
            }
            _ => {}
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                Self::walk_and_extract(
                    &child,
                    source,
                    file,
                    enclosing_fn,
                    enclosing_class,
                    namespace,
                    result,
                );
            }
        }
    }
}

impl TreeSitterParser for CSharpParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
            .map_err(|e| RpgError::tree_sitter_error(Path::new(""), e.to_string()))
    }

    fn parse_impl(source: &str, path: &Path, parser: &mut Parser) -> Result<ParseResult> {
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| RpgError::tree_sitter_error(path, "Failed to parse source"))?;

        let mut result = ParseResult::new(path.to_path_buf());
        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        let mut enclosing_fn = String::new();
        let mut enclosing_class: Option<String> = None;

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.is_named() {
                Self::walk_and_extract(
                    &child,
                    source_bytes,
                    path,
                    &mut enclosing_fn,
                    &mut enclosing_class,
                    "",
                    &mut result,
                );
            }
        }

        let ffi_bindings = FfiDetector::detect_csharp_pinvoke(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

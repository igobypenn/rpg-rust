use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Parser;

use crate::error::{Result, RpgError};
use crate::languages::builtins;
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::{collect_types_with_scoped, CachedParser, TreeSitterParser},
    docs::extract_documentation,
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ImportInfo, LanguageParser, ParseResult, TypeRefInfo,
    TypeRefKind,
};

pub struct GoParser {
    cached: CachedParser,
}

impl GoParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            cached: CachedParser::new::<Self>()?,
        })
    }

    fn extract_import(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        if node.kind() == "import_declaration" {
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                match child.kind() {
                    "import_spec" => {
                        return Self::extract_import_spec(&child, source, file);
                    }
                    "import_spec_list" => {
                        let mut list_cursor = child.walk();
                        for spec in child.children(&mut list_cursor) {
                            if spec.kind() == "import_spec" {
                                if let Some(import) = Self::extract_import_spec(&spec, source, file)
                                {
                                    return Some(import);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    fn extract_import_spec(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<ImportInfo> {
        let path = node
            .child_by_field_name("path")
            .map(|n| n.text(source))?
            .trim_matches('"')
            .to_string();

        let mut import = ImportInfo::new(&path);
        import.location = Some(node.to_location(file));

        if let Some(alias) = node.child_by_field_name("name") {
            let alias_text = alias.text(source);
            import.imported_names = vec![format!("{} as {}", path, alias_text)];
        } else {
            let name = path.rsplit('/').next().unwrap_or(&path);
            import.imported_names = vec![name.to_string()];
        }

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

        let mut def = DefinitionInfo::new("func", name);
        def.location = Some(node.to_location(file));
        def.is_public = name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let result_text = node
                .child_by_field_name("result")
                .map(|r| r.text(source))
                .unwrap_or_default();
            def.signature = Some(format!("{}{}{}", name, params_text, result_text));
        }

        if let Some(doc) = extract_documentation(node, source, "go") {
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

        let receiver = node.child_by_field_name("receiver").and_then(|r| {
            let mut cursor = r.walk();
            for child in r.children(&mut cursor) {
                if child.kind() == "parameter_declaration" {
                    if let Some(type_node) = child.child_by_field_name("type") {
                        return Some(type_node.text(source).to_string());
                    }
                }
            }
            None
        });

        let mut def = DefinitionInfo::new("method", name);
        def.location = Some(node.to_location(file));
        def.is_public = name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);

        if let Some(ref recv) = receiver {
            def.parent = Some(recv.clone());
            def.metadata.insert(
                "receiver".to_string(),
                serde_json::Value::String(recv.clone()),
            );
        }

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let result_text = node
                .child_by_field_name("result")
                .map(|r| r.text(source))
                .unwrap_or_default();
            let recv_text = receiver
                .as_ref()
                .map(|r| format!("({})", r))
                .unwrap_or_default();
            def.signature = Some(format!(
                "{}{}{}{}",
                name, recv_text, params_text, result_text
            ));
        }

        if let Some(doc) = extract_documentation(node, source, "go") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_type_decl(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "type_declaration" {
            return None;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_spec" {
                if let Some(def) = Self::extract_type_spec(&child, source, file) {
                    return Some(def);
                }
            }
        }
        None
    }

    fn extract_type_spec(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let type_node = node.child_by_field_name("type")?;
        let type_kind = type_node.kind();

        let kind = match type_kind {
            "struct_type" => "struct",
            "interface_type" => "interface",
            "func_type" => "func_type",
            _ => "type",
        };

        let mut def = DefinitionInfo::new(kind, name);
        def.location = Some(node.to_location(file));
        def.is_public = name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);

        if kind == "struct" {
            let fields = Self::extract_struct_fields(&type_node, source);
            if !fields.is_empty() {
                def.metadata.insert(
                    "fields".to_string(),
                    serde_json::Value::Array(
                        fields.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );
            }
        }

        if kind == "interface" {
            let methods = Self::extract_interface_methods(&type_node, source);
            if !methods.is_empty() {
                def.metadata.insert(
                    "methods".to_string(),
                    serde_json::Value::Array(
                        methods.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );
            }
        }

        if let Some(doc) = extract_documentation(node, source, "go") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_struct_fields(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut fields = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "field_declaration_list" {
                let mut field_cursor = child.walk();
                for field in child.children(&mut field_cursor) {
                    if field.kind() == "field_declaration" {
                        if let Some(name) = field.child_by_field_name("name") {
                            fields.push(name.text(source).to_string());
                        }
                    }
                }
            }
        }

        fields
    }

    fn extract_interface_methods(node: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut methods = Vec::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "method_spec" {
                if let Some(name) = child.child_by_field_name("name") {
                    methods.push(name.text(source).to_string());
                }
            }
        }

        methods
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
            "selector_expression" => {
                let obj = func.child_by_field_name("operand");
                let field = func.child_by_field_name("field");

                let receiver = obj.map(|o| o.text(source).to_string()).unwrap_or_default();
                let method = field
                    .map(|f| f.text(source).to_string())
                    .unwrap_or_default();

                if method
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                {
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

    fn extract_type_refs_from_func(
        node: &tree_sitter::Node,
        source: &[u8],
        fn_name: &str,
        result: &mut ParseResult,
    ) {
        let mut seen = HashSet::new();
        let mut types = Vec::new();

        if let Some(params) = node.child_by_field_name("parameters") {
            Self::extract_types_from_node(&params, source, fn_name, result, &mut seen, &mut types);
        }

        if let Some(recv) = node.child_by_field_name("receiver") {
            Self::extract_types_from_node(&recv, source, fn_name, result, &mut seen, &mut types);
        }

        if let Some(return_type) = node.child_by_field_name("result") {
            seen.clear();
            types.clear();
            collect_types_with_scoped(
                &return_type,
                source,
                &mut seen,
                &mut types,
                builtins::go::is_builtin,
                &["type_identifier"],
                &["qualified_type", "selector_expression"],
                &[
                    "pointer_type",
                    "slice_type",
                    "map_type",
                    "chan_type",
                    "generic_type",
                    "type_arguments",
                ],
            );
            for type_name in &types {
                result
                    .type_refs
                    .push(TypeRefInfo::ret(fn_name, type_name.clone()));
            }
        }
    }

    fn extract_types_from_node<'a>(
        node: &tree_sitter::Node,
        source: &'a [u8],
        fn_name: &str,
        result: &mut ParseResult,
        seen: &mut HashSet<&'a str>,
        types: &mut Vec<String>,
    ) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "parameter_declaration"
                || child.kind() == "variadic_parameter_declaration"
            {
                if let Some(type_node) = child.child_by_field_name("type") {
                    seen.clear();
                    types.clear();
                    collect_types_with_scoped(
                        &type_node,
                        source,
                        seen,
                        types,
                        builtins::go::is_builtin,
                        &["type_identifier"],
                        &["qualified_type", "selector_expression"],
                        &[
                            "pointer_type",
                            "slice_type",
                            "map_type",
                            "chan_type",
                            "generic_type",
                            "type_arguments",
                        ],
                    );
                    for type_name in types.iter() {
                        result
                            .type_refs
                            .push(TypeRefInfo::param(fn_name, type_name.clone()));
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
            "import_declaration" => {
                if let Some(import) = Self::extract_import(node, source, file) {
                    result.imports.push(import);
                }
            }
            "function_declaration" => {
                if let Some(def) = Self::extract_function(node, source, file) {
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
            "method_declaration" => {
                if let Some(def) = Self::extract_method(node, source, file) {
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
            "type_declaration" => {
                if let Some(def) = Self::extract_type_decl(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "call_expression" => {
                if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "short_var_declaration" | "var_declaration" | "const_declaration" => {
                Self::extract_var_types(node, source, enclosing_fn, result);
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

    fn extract_var_types(
        node: &tree_sitter::Node,
        source: &[u8],
        enclosing_fn: &str,
        result: &mut ParseResult,
    ) {
        let mut seen = HashSet::new();
        let mut types = Vec::new();

        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "var_spec" || child.kind() == "const_spec" {
                if let Some(type_node) = child.child_by_field_name("type") {
                    seen.clear();
                    types.clear();
                    collect_types_with_scoped(
                        &type_node,
                        source,
                        &mut seen,
                        &mut types,
                        builtins::go::is_builtin,
                        &["type_identifier"],
                        &["qualified_type", "selector_expression"],
                        &[
                            "pointer_type",
                            "slice_type",
                            "map_type",
                            "chan_type",
                            "generic_type",
                            "type_arguments",
                        ],
                    );
                    for type_name in &types {
                        result.type_refs.push(
                            TypeRefInfo::new(enclosing_fn, type_name.clone())
                                .with_kind(TypeRefKind::Local),
                        );
                    }
                }
            }
            if child.kind() == "assignment_statement" {
                if let Some(rhs) = child.child_by_field_name("right") {
                    seen.clear();
                    types.clear();
                    collect_types_with_scoped(
                        &rhs,
                        source,
                        &mut seen,
                        &mut types,
                        builtins::go::is_builtin,
                        &["type_identifier"],
                        &["qualified_type", "selector_expression"],
                        &[
                            "pointer_type",
                            "slice_type",
                            "map_type",
                            "chan_type",
                            "generic_type",
                            "type_arguments",
                        ],
                    );
                    for type_name in &types {
                        result.type_refs.push(
                            TypeRefInfo::new(enclosing_fn, type_name.clone())
                                .with_kind(TypeRefKind::Local),
                        );
                    }
                }
            }
        }
    }
}

impl TreeSitterParser for GoParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
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

        let mut ffi_bindings = FfiDetector::detect_cgo_exports(source, path);
        ffi_bindings.extend(FfiDetector::detect_cgo_imports(source, path));
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

impl LanguageParser for GoParser {
    fn language_name(&self) -> &str {
        "go"
    }

    fn file_extensions(&self) -> &[&str] {
        &["go"]
    }

    fn parse(&self, source: &str, path: &Path) -> Result<ParseResult> {
        self.cached.parse::<Self>(source, path)
    }
}

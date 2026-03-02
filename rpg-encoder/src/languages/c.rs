use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Parser;

use crate::error::{Result, RpgError};
use crate::languages::builtins;
use crate::parser::{
    base::{collect_types, CachedParser, TreeSitterParser},
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ImportInfo, LanguageParser, ParseResult, TypeRefInfo,
};

pub struct CParser {
    cached: CachedParser,
}

impl CParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            cached: CachedParser::new::<Self>()?,
        })
    }

    fn extract_include(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        if node.kind() != "preproc_include" {
            return None;
        }

        let path_node = node.child_by_field_name("path")?;
        let path_text = path_node.text(source);

        let (module_path, is_system) = if path_text.starts_with('"') {
            (path_text.trim_matches('"').to_string(), false)
        } else if path_text.starts_with('<') {
            (
                path_text
                    .trim_start_matches('<')
                    .trim_end_matches('>')
                    .to_string(),
                true,
            )
        } else {
            (path_text.to_string(), false)
        };

        let mut import = ImportInfo::new(&module_path);
        import.location = Some(node.to_location(file));
        import.imported_names = vec![module_path.clone()];
        import.is_glob = true;

        import
            .metadata
            .insert("system".to_string(), serde_json::Value::Bool(is_system));

        Some(import)
    }

    fn extract_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "function_definition" {
            return None;
        }

        let decl = node.child_by_field_name("declarator")?;

        let name = Self::extract_fn_name_from_declarator(&decl, source)?;

        let mut def = DefinitionInfo::new("fn", &name);
        def.location = Some(node.to_location(file));

        if let Some(type_node) = node.child_by_field_name("type") {
            let return_type = type_node.text(source);
            let params_text = Self::extract_params_text(&decl, source);
            def.signature = Some(format!("{} {}{}", return_type, name, params_text));
        }

        let body = node.child_by_field_name("body");
        def.metadata.insert(
            "has_body".to_string(),
            serde_json::Value::Bool(body.is_some()),
        );

        def.is_public = true;

        Some(def)
    }

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

    fn extract_union(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "union_specifier" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("union", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        Some(def)
    }

    fn extract_enum(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "enum_specifier" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("enum", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        Some(def)
    }

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

    fn extract_params_text(node: &tree_sitter::Node, source: &[u8]) -> String {
        if node.kind() == "function_declarator" {
            if let Some(params) = node.child_by_field_name("parameters") {
                return params.text(source).to_string();
            }
        }
        String::new()
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
            if let Some(func_decl) = Self::find_function_declarator(&decl) {
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

    fn find_function_declarator<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        if node.kind() == "function_declarator" {
            return Some(*node);
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                if let Some(found) = Self::find_function_declarator(&child) {
                    return Some(found);
                }
            }
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
            "preproc_include" => {
                if let Some(import) = Self::extract_include(node, source, file) {
                    result.imports.push(import);
                }
            }
            "function_definition" => {
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

impl LanguageParser for CParser {
    fn language_name(&self) -> &str {
        "c"
    }

    fn file_extensions(&self) -> &[&str] {
        &["c", "h"]
    }

    fn parse(&self, source: &str, path: &Path) -> Result<ParseResult> {
        self.cached.parse::<Self>(source, path)
    }
}

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

pub struct ScalaParser {
    cached: CachedParser,
}

impl ScalaParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            cached: CachedParser::new::<Self>()?,
        })
    }

    fn extract_package(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        if node.kind() != "package_clause" {
            return None;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "package_identifier" || child.kind() == "identifier" {
                return Some(child.text(source).to_string());
            }
        }

        None
    }

    fn extract_import(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        if node.kind() != "import_declaration" && node.kind() != "import" {
            return None;
        }

        let text = node.text(source);
        let text = text.trim_start_matches("import ").trim();

        let mut module_path = text.to_string();
        let mut is_wildcard = false;
        let mut imported_names: Vec<String> = Vec::new();

        if module_path.ends_with("._") || module_path.ends_with(".*") {
            is_wildcard = true;
            module_path = module_path
                .trim_end_matches("._")
                .trim_end_matches(".*")
                .to_string();
        } else if module_path.contains('{') && module_path.contains('}') {
            if let Some(start) = module_path.find('{') {
                if let Some(end) = module_path.find('}') {
                    let selectors = &module_path[start + 1..end];
                    imported_names = selectors
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    module_path = module_path[..start].trim_end_matches('.').to_string();
                }
            }
        } else if let Some(last_dot) = module_path.rfind('.') {
            imported_names.push(module_path[last_dot + 1..].to_string());
        }

        if module_path.is_empty() {
            return None;
        }

        let mut import = ImportInfo::new(&module_path);
        import.location = Some(node.to_location(file));
        import.is_glob = is_wildcard;

        if !imported_names.is_empty() {
            import.imported_names = imported_names;
        }

        Some(import)
    }

    fn extract_class(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        package: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "class_definition" {
            return None;
        }

        let name = Self::extract_definition_name(node, source)?;

        let mut def = DefinitionInfo::new("class", name);
        def.location = Some(node.to_location(file));

        if !package.is_empty() {
            def.metadata.insert(
                "package".to_string(),
                serde_json::Value::String(package.to_string()),
            );
        }

        if Self::is_case_class(node, source) {
            def.metadata
                .insert("case".to_string(), serde_json::Value::Bool(true));
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

        if let Some(doc) = extract_documentation(node, source, "scala") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_trait(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        package: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "trait_definition" {
            return None;
        }

        let name = Self::extract_definition_name(node, source)?;

        let mut def = DefinitionInfo::new("trait", name);
        def.location = Some(node.to_location(file));

        if !package.is_empty() {
            def.metadata.insert(
                "package".to_string(),
                serde_json::Value::String(package.to_string()),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "scala") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_object(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        package: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "object_definition" {
            return None;
        }

        let name = Self::extract_definition_name(node, source)?;

        let mut def = DefinitionInfo::new("object", name);
        def.location = Some(node.to_location(file));

        if !package.is_empty() {
            def.metadata.insert(
                "package".to_string(),
                serde_json::Value::String(package.to_string()),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "scala") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_enum(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        package: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "enum_definition" {
            return None;
        }

        let name = Self::extract_definition_name(node, source)?;

        let mut def = DefinitionInfo::new("enum", name);
        def.location = Some(node.to_location(file));

        if !package.is_empty() {
            def.metadata.insert(
                "package".to_string(),
                serde_json::Value::String(package.to_string()),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "scala") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "function_definition" && node.kind() != "function_declaration" {
            return None;
        }

        let name = Self::extract_definition_name(node, source)?;

        let mut def = DefinitionInfo::new("function", &name);
        def.location = Some(node.to_location(file));
        def.parent = if enclosing.is_empty() {
            None
        } else {
            Some(enclosing.to_string())
        };

        if let Some(params) = Self::extract_function_params(node, source) {
            if let Some(return_type) = Self::extract_return_type(node, source) {
                def.signature = Some(format!("def {}({}): {}", name, params, return_type));
            } else {
                def.signature = Some(format!("def {}({})", name, params));
            }
        }

        if let Some(doc) = extract_documentation(node, source, "scala") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_definition_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" {
                return Some(child.text(source).to_string());
            }
        }
        None
    }

    fn extract_type_params(node: &tree_sitter::Node, source: &[u8]) -> Option<Vec<String>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_parameters" {
                let mut tp_cursor = child.walk();
                let params: Vec<String> = child
                    .children(&mut tp_cursor)
                    .filter(|c| c.kind() == "type_parameter" || c.kind() == "identifier")
                    .map(|c| c.text(source).to_string())
                    .collect();
                if !params.is_empty() {
                    return Some(params);
                }
            }
        }
        None
    }

    fn is_case_class(node: &tree_sitter::Node, source: &[u8]) -> bool {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifier" {
                let text = child.text(source);
                if text == "case" {
                    return true;
                }
            }
        }
        false
    }

    fn extract_function_params(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "parameters" {
                return Some(child.text(source).to_string());
            }
        }
        None
    }

    fn extract_return_type(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "return_type" {
                return Some(
                    child
                        .text(source)
                        .trim_start_matches(':')
                        .trim()
                        .to_string(),
                );
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
        if node.kind() != "call_expression" && node.kind() != "method_call" {
            return None;
        }

        let name = if node.kind() == "method_call" {
            Self::extract_method_call_name(node, source)
        } else {
            Self::extract_simple_call_name(node, source)
        }?;

        let location = node.to_location(file);

        let call = CallInfo::new(enclosing_fn, &name)
            .with_kind(CallKind::Method)
            .with_location(location);

        Some(call)
    }

    fn extract_method_call_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "identifier" || child.kind() == "operator_identifier" {
                return Some(child.text(source).to_string());
            }
        }
        None
    }

    fn extract_simple_call_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        if let Some(func) = node.child_by_field_name("function") {
            let text = func.text(source);
            if let Some(last) = text.split('.').next_back() {
                return Some(last.to_string());
            }
            return Some(text.to_string());
        }
        None
    }

    fn extract_type_refs_from_function(
        node: &tree_sitter::Node,
        source: &[u8],
        fn_name: &str,
        result: &mut ParseResult,
    ) {
        let mut seen = HashSet::new();
        let mut types = Vec::new();

        if let Some(return_type) = Self::find_return_type_node(node) {
            seen.clear();
            types.clear();
            collect_types_with_scoped(
                &return_type,
                source,
                &mut seen,
                &mut types,
                builtins::scala::is_builtin,
                &["identifier", "type_identifier", "stable_type_identifier"],
                &[],
                &["type_argument", "type_arguments"],
            );
            for type_name in &types {
                result
                    .type_refs
                    .push(TypeRefInfo::ret(fn_name, type_name.clone()));
            }
        }

        if let Some(params) = Self::find_params_node(node) {
            let mut cursor = params.walk();
            for param in params.children(&mut cursor) {
                if param.kind() == "parameter" || param.kind() == "class_parameter" {
                    seen.clear();
                    types.clear();
                    collect_types_with_scoped(
                        &param,
                        source,
                        &mut seen,
                        &mut types,
                        builtins::scala::is_builtin,
                        &["identifier", "type_identifier", "stable_type_identifier"],
                        &[],
                        &["type_argument", "type_arguments"],
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

    #[allow(clippy::manual_find)]
    fn find_return_type_node<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "return_type" {
                return Some(child);
            }
        }
        None
    }

    #[allow(clippy::manual_find)]
    fn find_params_node<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "parameters" {
                return Some(child);
            }
        }
        None
    }

    #[allow(clippy::manual_find)]
    fn find_type_annotation<'a>(node: &tree_sitter::Node<'a>) -> Option<tree_sitter::Node<'a>> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_annotation" {
                return Some(child);
            }
        }
        None
    }

    fn walk_and_extract(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &mut String,
        enclosing_type: &mut Option<String>,
        package: &str,
        result: &mut ParseResult,
    ) {
        match node.kind() {
            "package_clause" => {}
            "import_declaration" | "import" => {
                if let Some(import) = Self::extract_import(node, source, file) {
                    result.imports.push(import);
                }
            }
            "class_definition" => {
                if let Some(def) = Self::extract_class(node, source, file, package) {
                    let class_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_type = enclosing_type.clone();
                    *enclosing_type = Some(class_name);

                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.is_named() {
                            Self::walk_and_extract(
                                &child,
                                source,
                                file,
                                enclosing_fn,
                                enclosing_type,
                                package,
                                result,
                            );
                        }
                    }

                    *enclosing_type = prev_type;
                    return;
                }
            }
            "trait_definition" => {
                if let Some(def) = Self::extract_trait(node, source, file, package) {
                    result.definitions.push(def);
                }
            }
            "object_definition" => {
                if let Some(def) = Self::extract_object(node, source, file, package) {
                    let obj_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_type = enclosing_type.clone();
                    *enclosing_type = Some(obj_name);

                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.is_named() {
                            Self::walk_and_extract(
                                &child,
                                source,
                                file,
                                enclosing_fn,
                                enclosing_type,
                                package,
                                result,
                            );
                        }
                    }

                    *enclosing_type = prev_type;
                    return;
                }
            }
            "enum_definition" => {
                if let Some(def) = Self::extract_enum(node, source, file, package) {
                    result.definitions.push(def);
                }
            }
            "function_definition" | "function_declaration" => {
                let enclosing = enclosing_type.as_deref().unwrap_or("");
                if let Some(def) = Self::extract_function(node, source, file, enclosing) {
                    let fn_name = def.name.clone();
                    Self::extract_type_refs_from_function(node, source, &fn_name, result);
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
                        if child.is_named() {
                            Self::walk_and_extract(
                                &child,
                                source,
                                file,
                                enclosing_fn,
                                enclosing_type,
                                package,
                                result,
                            );
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "call_expression" | "method_call" => {
                if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "val_definition" | "var_definition" => {
                if let Some(type_node) = Self::find_type_annotation(node) {
                    let mut seen = HashSet::new();
                    let mut types = Vec::new();
                    collect_types_with_scoped(
                        &type_node,
                        source,
                        &mut seen,
                        &mut types,
                        builtins::scala::is_builtin,
                        &["identifier", "type_identifier", "stable_type_identifier"],
                        &[],
                        &["type_argument", "type_arguments"],
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
                Self::walk_and_extract(
                    &child,
                    source,
                    file,
                    enclosing_fn,
                    enclosing_type,
                    package,
                    result,
                );
            }
        }
    }
}

impl TreeSitterParser for ScalaParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_scala::LANGUAGE.into())
            .map_err(|e| RpgError::tree_sitter_error(Path::new(""), e.to_string()))
    }

    fn parse_impl(source: &str, path: &Path, parser: &mut Parser) -> Result<ParseResult> {
        let tree = parser
            .parse(source, None)
            .ok_or_else(|| RpgError::tree_sitter_error(path, "Failed to parse source"))?;

        let mut result = ParseResult::new(path.to_path_buf());
        let root = tree.root_node();
        let source_bytes = source.as_bytes();

        let mut package = String::new();

        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "package_clause" {
                package = Self::extract_package(&child, source_bytes).unwrap_or_default();
            }
        }

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
                    &package,
                    &mut result,
                );
            }
        }

        let ffi_bindings = FfiDetector::detect_scala_ffi(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

impl LanguageParser for ScalaParser {
    fn language_name(&self) -> &str {
        "scala"
    }

    fn file_extensions(&self) -> &[&str] {
        &["scala", "sc"]
    }

    fn parse(&self, source: &str, path: &Path) -> Result<ParseResult> {
        self.cached.parse::<Self>(source, path)
    }
}

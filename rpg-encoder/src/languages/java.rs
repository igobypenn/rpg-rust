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

pub struct JavaParser {
    cached: CachedParser,
}

impl JavaParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            cached: CachedParser::new::<Self>()?,
        })
    }

    fn extract_modifiers(node: &tree_sitter::Node, source: &[u8]) -> (bool, bool) {
        let mut is_public = false;
        let mut is_native = false;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "modifiers" {
                let mut mod_cursor = child.walk();
                for modifier in child.children(&mut mod_cursor) {
                    let text = modifier.text(source);
                    match text {
                        "public" | "protected" => is_public = true,
                        "native" => is_native = true,
                        _ => {}
                    }
                }
            }
        }

        (is_public, is_native)
    }

    fn extract_package(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        if node.kind() != "package_declaration" {
            return None;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
                return Some(child.text(source).to_string());
            }
        }

        None
    }

    fn extract_import(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        if node.kind() != "import_declaration" {
            return None;
        }

        let mut cursor = node.walk();
        let mut module_path = String::new();
        let mut is_wildcard = false;

        for child in node.children(&mut cursor) {
            if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
                module_path = child.text(source).to_string();
            }
            if child.kind() == "*" {
                is_wildcard = true;
            }
        }

        if module_path.is_empty() {
            return None;
        }

        let mut import = ImportInfo::new(&module_path);
        import.location = Some(node.to_location(file));
        import.is_glob = is_wildcard;

        let name = module_path.rsplit('.').next().unwrap_or(&module_path);
        if !is_wildcard {
            import.imported_names = vec![name.to_string()];
        }

        Some(import)
    }

    fn extract_class(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        package: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "class_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("class", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if !package.is_empty() {
            def.metadata.insert(
                "package".to_string(),
                serde_json::Value::String(package.to_string()),
            );
        }

        if let Some(super_class) = node.child_by_field_name("superclass") {
            let super_name = super_class.text(source);
            def.metadata.insert(
                "extends".to_string(),
                serde_json::Value::String(super_name.to_string()),
            );
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "super_interfaces" {
                let mut iface_cursor = child.walk();
                let interfaces: Vec<String> = child
                    .children(&mut iface_cursor)
                    .filter(|c| c.kind() == "type_list")
                    .flat_map(|tl| {
                        let mut tl_cursor = tl.walk();
                        tl.children(&mut tl_cursor)
                            .filter(|c| c.kind() == "type_identifier")
                            .map(|c| c.text(source).to_string())
                            .collect::<Vec<_>>()
                    })
                    .collect();
                if !interfaces.is_empty() {
                    def.metadata.insert(
                        "implements".to_string(),
                        serde_json::Value::Array(
                            interfaces
                                .into_iter()
                                .map(serde_json::Value::String)
                                .collect(),
                        ),
                    );
                }
            }
        }

        if let Some(doc) = extract_documentation(node, source, "java") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_interface(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        package: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "interface_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("interface", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if !package.is_empty() {
            def.metadata.insert(
                "package".to_string(),
                serde_json::Value::String(package.to_string()),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "java") {
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
        if node.kind() != "enum_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let (is_public, _) = Self::extract_modifiers(node, source);

        let mut def = DefinitionInfo::new("enum", name);
        def.location = Some(node.to_location(file));
        def.is_public = is_public;

        if !package.is_empty() {
            def.metadata.insert(
                "package".to_string(),
                serde_json::Value::String(package.to_string()),
            );
        }

        if let Some(doc) = extract_documentation(node, source, "java") {
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

        let (is_public, is_native) = Self::extract_modifiers(node, source);

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

        if is_native {
            def.metadata
                .insert("native".to_string(), serde_json::Value::Bool(true));
        }

        if let Some(doc) = extract_documentation(node, source, "java") {
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

        let (is_public, _) = Self::extract_modifiers(node, source);

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
        if node.kind() != "method_invocation" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let location = node.to_location(file);

        let receiver = node
            .child_by_field_name("object")
            .map(|o| o.text(source).to_string());

        let call_kind = if receiver.is_some() {
            CallKind::Method
        } else {
            CallKind::Direct
        };

        let call = CallInfo::new(enclosing_fn, name)
            .with_kind(call_kind)
            .with_location(location);

        Some(if let Some(rec) = receiver {
            call.with_receiver(rec)
        } else {
            call
        })
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
                builtins::java::is_builtin,
                &["type_identifier"],
                &["scoped_type_identifier"],
                &["generic_type", "array_type"],
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
                if param.kind() == "formal_parameter" {
                    if let Some(type_node) = param.child_by_field_name("type") {
                        seen.clear();
                        types.clear();
                        collect_types_with_scoped(
                            &type_node,
                            source,
                            &mut seen,
                            &mut types,
                            builtins::java::is_builtin,
                            &["type_identifier"],
                            &["scoped_type_identifier"],
                            &["generic_type", "array_type"],
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
        package: &str,
        result: &mut ParseResult,
    ) {
        match node.kind() {
            "package_declaration" => {}
            "import_declaration" => {
                if let Some(import) = Self::extract_import(node, source, file) {
                    result.imports.push(import);
                }
            }
            "class_declaration" => {
                if let Some(def) = Self::extract_class(node, source, file, package) {
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
                                    package,
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
                if let Some(def) = Self::extract_interface(node, source, file, package) {
                    result.definitions.push(def);
                }
            }
            "enum_declaration" => {
                if let Some(def) = Self::extract_enum(node, source, file, package) {
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
                                        package,
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
            "field_declaration" => {
                if let Some(class) = enclosing_class.as_ref() {
                    if let Some(def) = Self::extract_field(node, source, file, class) {
                        result.definitions.push(def);
                    }
                }
            }
            "method_invocation" => {
                if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "local_variable_declaration" => {
                if let Some(type_node) = node.child_by_field_name("type") {
                    let mut seen = HashSet::new();
                    let mut types = Vec::new();
                    collect_types_with_scoped(
                        &type_node,
                        source,
                        &mut seen,
                        &mut types,
                        builtins::java::is_builtin,
                        &["type_identifier"],
                        &["scoped_type_identifier"],
                        &["generic_type", "array_type"],
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
                    enclosing_class,
                    package,
                    result,
                );
            }
        }
    }
}

impl TreeSitterParser for JavaParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_java::LANGUAGE.into())
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
            if child.kind() == "package_declaration" {
                package = Self::extract_package(&child, source_bytes).unwrap_or_default();
            }
        }

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
                    &package,
                    &mut result,
                );
            }
        }

        let ffi_bindings = FfiDetector::detect_java_jni(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

impl LanguageParser for JavaParser {
    fn language_name(&self) -> &str {
        "java"
    }

    fn file_extensions(&self) -> &[&str] {
        &["java"]
    }

    fn parse(&self, source: &str, path: &Path) -> Result<ParseResult> {
        self.cached.parse::<Self>(source, path)
    }
}

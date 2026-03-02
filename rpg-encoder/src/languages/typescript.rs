use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Parser;

use crate::error::{Result, RpgError};
use crate::languages::builtins;
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::{collect_types, CachedParser, TreeSitterParser},
    docs::extract_documentation,
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ImportInfo, LanguageParser, ParseResult, TypeRefInfo,
};

pub struct TypeScriptParser {
    cached: CachedParser,
}

impl TypeScriptParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            cached: CachedParser::new::<Self>()?,
        })
    }

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
                let source_node = node.child_by_field_name("source")?;
                let module_path = source_node
                    .text(source)
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string();

                let mut import = ImportInfo::new(&module_path);
                import.location = Some(node.to_location(file));

                let mut names = Vec::new();
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "import_clause" {
                        let mut clause_cursor = child.walk();
                        for clause_child in child.children(&mut clause_cursor) {
                            match clause_child.kind() {
                                "identifier" => {
                                    names.push(clause_child.text(source).to_string());
                                }
                                "named_imports" => {
                                    let mut spec_cursor = clause_child.walk();
                                    for spec in clause_child.children(&mut spec_cursor) {
                                        if spec.kind() == "import_specifier" {
                                            if let Some(name) = spec.child_by_field_name("name") {
                                                let name_text = name.text(source);
                                                if let Some(alias) =
                                                    spec.child_by_field_name("alias")
                                                {
                                                    names.push(format!(
                                                        "{} as {}",
                                                        name_text,
                                                        alias.text(source)
                                                    ));
                                                } else {
                                                    names.push(name_text.to_string());
                                                }
                                            }
                                        }
                                    }
                                }
                                "namespace_import" => {
                                    if let Some(name) = clause_child.child_by_field_name("name") {
                                        names.push(format!("* as {}", name.text(source)));
                                        import.is_glob = true;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                import.imported_names = names;
                Some(import)
            }
            "export_statement" => Self::extract_reexport(node, source, file),
            _ => None,
        }
    }

    fn extract_reexport(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<ImportInfo> {
        let source_node = node.child_by_field_name("source")?;
        let module_path = source_node
            .text(source)
            .trim_matches('"')
            .trim_matches('\'')
            .to_string();

        let mut import = ImportInfo::new(&module_path);
        import.location = Some(node.to_location(file));
        import.is_glob = true;

        Some(import)
    }

    fn extract_require_call(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<ImportInfo> {
        if node.kind() != "call_expression" {
            return None;
        }

        let func = node.child_by_field_name("function")?;
        if func.kind() != "identifier" || func.text(source) != "require" {
            return None;
        }

        let args = node.child_by_field_name("arguments")?;
        let mut cursor = args.walk();
        let first_arg = args.children(&mut cursor).find(|c| c.is_named())?;

        if first_arg.kind() == "string" || first_arg.kind() == "template_string" {
            let arg_text = first_arg.text(source);
            let module_path = arg_text.trim_matches('"').trim_matches('\'').to_string();

            let mut import = ImportInfo::new(&module_path);
            import.location = Some(node.to_location(file));

            let parent = node.parent();
            if let Some(p) = parent {
                if p.kind() == "variable_declarator" {
                    if let Some(name) = p.child_by_field_name("name") {
                        import.imported_names = vec![name.text(source).to_string()];
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
        if node.kind() != "function_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("function", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let return_type = node
                .child_by_field_name("return_type")
                .map(|r| r.text(source))
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

        if let Some(doc) = extract_documentation(node, source, "typescript") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_arrow_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "arrow_function" {
            return None;
        }

        let parent = node.parent()?;
        let name = match parent.kind() {
            "variable_declarator" => parent.child_by_field_name("name").map(|n| n.text(source))?,
            "assignment_expression" => {
                parent.child_by_field_name("left").map(|n| n.text(source))?
            }
            _ => return None,
        };

        let mut def = DefinitionInfo::new("arrow", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let return_type = node
                .child_by_field_name("return_type")
                .map(|r| r.text(source))
                .unwrap_or_default();
            def.signature = Some(format!("{}{}{}", name, params_text, return_type));
        }

        Some(def)
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

        let mut def = DefinitionInfo::new("class", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        if let Some(parent) = node.child_by_field_name("parent_class") {
            let parent_name = parent.text(source);
            def.metadata.insert(
                "extends".to_string(),
                serde_json::Value::String(parent_name.to_string()),
            );
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "implements_clause" {
                let mut impl_cursor = child.walk();
                let interfaces: Vec<String> = child
                    .children(&mut impl_cursor)
                    .filter(|c| c.kind() == "type_identifier" || c.kind() == "generic_type")
                    .map(|c| c.text(source).to_string())
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

        let body = node.child_by_field_name("body");
        if let Some(body) = body {
            let methods = Self::extract_class_methods(&body, source);
            if !methods.is_empty() {
                def.metadata.insert(
                    "methods".to_string(),
                    serde_json::Value::Array(
                        methods.into_iter().map(serde_json::Value::String).collect(),
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

        if let Some(doc) = extract_documentation(node, source, "typescript") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_interface(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "interface_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("interface", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "extends_clause" {
                let mut ext_cursor = child.walk();
                let parents: Vec<String> = child
                    .children(&mut ext_cursor)
                    .filter(|c| c.kind() == "type_identifier")
                    .map(|c| c.text(source).to_string())
                    .collect();
                if !parents.is_empty() {
                    def.metadata.insert(
                        "extends".to_string(),
                        serde_json::Value::Array(
                            parents.into_iter().map(serde_json::Value::String).collect(),
                        ),
                    );
                }
            }
        }

        if let Some(doc) = extract_documentation(node, source, "typescript") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_type_alias(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "type_alias_declaration" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("type", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        if let Some(doc) = extract_documentation(node, source, "typescript") {
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

        let mut def = DefinitionInfo::new("enum", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        if let Some(doc) = extract_documentation(node, source, "typescript") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_class_methods(body: &tree_sitter::Node, source: &[u8]) -> Vec<String> {
        let mut methods = Vec::new();
        let mut cursor = body.walk();

        for child in body.children(&mut cursor) {
            if child.kind() == "method_definition" || child.kind() == "public_field_definition" {
                if let Some(name) = child.child_by_field_name("name") {
                    methods.push(name.text(source).to_string());
                }
            }
        }

        methods
    }

    fn extract_method(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_class: &str,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "method_definition" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("method", name);
        def.location = Some(node.to_location(file));
        def.parent = Some(enclosing_class.to_string());
        def.is_public = name != "constructor";

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            let return_type = node
                .child_by_field_name("return_type")
                .map(|r| r.text(source))
                .unwrap_or_default();
            def.signature = Some(format!("{}{}{}", name, params_text, return_type));
        }

        if let Some(doc) = extract_documentation(node, source, "typescript") {
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
        if node.kind() != "call_expression" {
            return None;
        }

        let func = node.child_by_field_name("function")?;

        let location = node.to_location(file);

        let (callee, receiver, call_kind) = match func.kind() {
            "identifier" => {
                let name = func.text(source);
                if name == "require" {
                    return None;
                }
                (name.to_string(), None, CallKind::Direct)
            }
            "member_expression" => {
                let obj = func.child_by_field_name("object");
                let prop = func.child_by_field_name("property");

                let receiver = obj.map(|o| o.text(source).to_string()).unwrap_or_default();
                let method = prop.map(|p| p.text(source).to_string()).unwrap_or_default();

                (method, Some(receiver), CallKind::Method)
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

    fn extract_type_refs(
        node: &tree_sitter::Node,
        source: &[u8],
        fn_name: &str,
        result: &mut ParseResult,
    ) {
        let mut seen = HashSet::new();
        let mut types = Vec::new();

        if let Some(params) = node.child_by_field_name("parameters") {
            let mut cursor = params.walk();
            for param in params.children(&mut cursor) {
                if param.kind() == "required_parameter"
                    || param.kind() == "optional_parameter"
                    || param.kind() == "rest_parameter"
                {
                    if let Some(pattern) = param.child_by_field_name("pattern") {
                        if let Some(type_node) = pattern.child_by_field_name("type") {
                            seen.clear();
                            types.clear();
                            collect_types(
                                &type_node,
                                source,
                                &mut seen,
                                &mut types,
                                builtins::typescript::is_builtin,
                                &["type_identifier"],
                                &[],
                            );
                            for type_name in &types {
                                result
                                    .type_refs
                                    .push(TypeRefInfo::param(fn_name, type_name.clone()));
                            }
                        }
                    }
                    if let Some(type_node) = param.child_by_field_name("type") {
                        seen.clear();
                        types.clear();
                        collect_types(
                            &type_node,
                            source,
                            &mut seen,
                            &mut types,
                            builtins::typescript::is_builtin,
                            &["type_identifier"],
                            &[],
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

        if let Some(return_type) = node.child_by_field_name("return_type") {
            seen.clear();
            types.clear();
            collect_types(
                &return_type,
                source,
                &mut seen,
                &mut types,
                builtins::typescript::is_builtin,
                &["type_identifier"],
                &[],
            );
            for type_name in &types {
                result
                    .type_refs
                    .push(TypeRefInfo::param(fn_name, type_name.clone()));
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
        match node.kind() {
            "import_statement" | "export_statement" => {
                if let Some(import) = Self::extract_import(node, source, file) {
                    result.imports.push(import);
                }
            }
            "call_expression" => {
                if let Some(import) = Self::extract_require_call(node, source, file) {
                    result.imports.push(import);
                } else if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "function_declaration" => {
                if let Some(def) = Self::extract_function(node, source, file) {
                    let fn_name = def.name.clone();
                    Self::extract_type_refs(node, source, &fn_name, result);
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
                                    result,
                                );
                            }
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "arrow_function" => {
                if let Some(def) = Self::extract_arrow_function(node, source, file) {
                    let fn_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    if let Some(body) = node.child_by_field_name("body") {
                        if body.kind() == "statement_block" {
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
                    }

                    *enclosing_fn = prev_fn;
                }
            }
            "class_declaration" => {
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
            "interface_declaration" => {
                if let Some(def) = Self::extract_interface(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "type_alias_declaration" => {
                if let Some(def) = Self::extract_type_alias(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "enum_declaration" => {
                if let Some(def) = Self::extract_enum(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "method_definition" => {
                if let Some(class) = enclosing_class.as_ref() {
                    if let Some(def) = Self::extract_method(node, source, file, class) {
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

impl TreeSitterParser for TypeScriptParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into())
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

        let ffi_bindings = FfiDetector::detect_node_native(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

impl LanguageParser for TypeScriptParser {
    fn language_name(&self) -> &str {
        "typescript"
    }

    fn file_extensions(&self) -> &[&str] {
        &["ts", "tsx"]
    }

    fn parse(&self, source: &str, path: &Path) -> Result<ParseResult> {
        self.cached.parse::<Self>(source, path)
    }
}

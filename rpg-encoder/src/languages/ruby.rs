use std::path::Path;

use tree_sitter::Parser;

use crate::define_parser;
use crate::error::{Result, RpgError};
use crate::languages::builtins;
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::TreeSitterParser, docs::extract_documentation, helpers::TsNodeExt, CallInfo, CallKind,
    DefinitionInfo, ImportInfo, ParseResult, TypeRefInfo, TypeRefKind,
};

define_parser!(RubyParser, "ruby", &["rb", "rake"]);

impl RubyParser {
    fn extract_require(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        if node.kind() != "call" {
            return None;
        }

        let method = node.child_by_field_name("method");

        let method_name = method.map(|m| m.text(source))?;

        match method_name {
            "require" | "require_relative" | "load" => {
                let is_relative = method_name == "require_relative";
                let is_load = method_name == "load";

                if let Some(args) = node.child_by_field_name("arguments") {
                    let mut cursor = args.walk();
                    for arg in args.children(&mut cursor) {
                        if arg.kind() == "string" || arg.kind() == "simple_symbol" {
                            let arg_text = arg.text(source);
                            let module_path = arg_text
                                .trim_matches('"')
                                .trim_matches('\'')
                                .trim_start_matches(':')
                                .to_string();

                            let mut import = ImportInfo::new(&module_path);
                            import.location = Some(node.to_location(file));
                            import.is_glob = false;
                            import.imported_names = vec![module_path.clone()];

                            import.metadata.insert(
                                "require_type".to_string(),
                                serde_json::Value::String(method_name.to_string()),
                            );

                            if is_relative {
                                import
                                    .metadata
                                    .insert("relative".to_string(), serde_json::Value::Bool(true));
                            }
                            if is_load {
                                import
                                    .metadata
                                    .insert("load".to_string(), serde_json::Value::Bool(true));
                            }

                            return Some(import);
                        }
                    }
                }
            }
            _ => {}
        }

        None
    }

    fn extract_method(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "method" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("method", name);
        def.location = Some(node.to_location(file));

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            def.signature = Some(format!("{}{}", name, params_text));
        }

        def.is_public = !name.ends_with('?') && !name.ends_with('!') && !name.starts_with('_');

        if let Some(doc) = extract_documentation(node, source, "ruby") {
            def.doc = Some(doc);
        }

        if node.child_by_field_name("body").is_some() {
            def.metadata
                .insert("has_body".to_string(), serde_json::Value::Bool(true));
        }

        Some(def)
    }

    fn extract_singleton_method(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "singleton_method" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let object = node
            .child_by_field_name("object")
            .map(|o| o.text(source).to_string());

        let mut def = DefinitionInfo::new("class_method", name);
        def.location = Some(node.to_location(file));

        if let Some(ref obj) = object {
            def.parent = Some(obj.clone());
            def.metadata
                .insert("target".to_string(), serde_json::Value::String(obj.clone()));
        }

        if let Some(params) = node.child_by_field_name("parameters") {
            let params_text = params.text(source);
            def.signature = Some(format!(
                "{}.{}{}",
                object.unwrap_or_default(),
                name,
                params_text
            ));
        }

        if let Some(doc) = extract_documentation(node, source, "ruby") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_class(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "class" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("class", name);
        def.location = Some(node.to_location(file));

        if let Some(superclass) = node.child_by_field_name("superclass") {
            let super_name = superclass.text(source);
            def.metadata.insert(
                "superclass".to_string(),
                serde_json::Value::String(super_name.to_string()),
            );
        }

        def.is_public = !name.starts_with('_');

        if let Some(doc) = extract_documentation(node, source, "ruby") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_module(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "module" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("module", name);
        def.location = Some(node.to_location(file));
        def.is_public = !name.starts_with('_');

        if let Some(doc) = extract_documentation(node, source, "ruby") {
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

        let method = node.child_by_field_name("method")?;
        let method_name = method.text(source);

        if ["require", "require_relative", "load", "puts", "print", "p"].contains(&method_name) {
            return None;
        }

        let location = node.to_location(file);
        let receiver = node.child_by_field_name("receiver");

        let (callee, receiver_text, call_kind) = if let Some(recv) = receiver {
            let recv_text = recv.text(source).to_string();
            (method_name.to_string(), Some(recv_text), CallKind::Method)
        } else {
            (method_name.to_string(), None, CallKind::Direct)
        };

        if callee.is_empty() {
            return None;
        }

        let call = CallInfo::new(enclosing_fn, callee)
            .with_kind(call_kind)
            .with_location(location);

        Some(if let Some(rec) = receiver_text {
            call.with_receiver(rec)
        } else {
            call
        })
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
            "call" => {
                if let Some(import) = Self::extract_require(node, source, file) {
                    result.imports.push(import);
                } else if let Some(call) = Self::extract_call(node, source, file, enclosing_fn) {
                    result.calls.push(call);
                }
            }
            "method" => {
                if let Some(mut def) = Self::extract_method(node, source, file) {
                    let fn_name = def.name.clone();

                    if let Some(ref class) = *enclosing_class {
                        def.parent = Some(class.clone());
                    }

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
                                enclosing_class,
                                result,
                            );
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "singleton_method" => {
                if let Some(def) = Self::extract_singleton_method(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "class" => {
                if let Some(mut def) = Self::extract_class(node, source, file) {
                    let class_name = def.name.clone();

                    if let Some(ref parent_class) = *enclosing_class {
                        def.parent = Some(parent_class.clone());
                    }

                    result.definitions.push(def);

                    let prev_class = enclosing_class.clone();
                    *enclosing_class = Some(class_name);

                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
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

                    *enclosing_class = prev_class;
                    return;
                }
            }
            "module" => {
                if let Some(mut def) = Self::extract_module(node, source, file) {
                    let module_name = def.name.clone();

                    if let Some(ref parent) = *enclosing_class {
                        def.parent = Some(parent.clone());
                    }

                    result.definitions.push(def);

                    let prev_class = enclosing_class.clone();
                    *enclosing_class = Some(module_name);

                    let mut cursor = node.walk();
                    for child in node.children(&mut cursor) {
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

                    *enclosing_class = prev_class;
                    return;
                }
            }
            "identifier" if !enclosing_fn.is_empty() => {
                let text = node.text(source);
                if text
                    .chars()
                    .next()
                    .map(|c| c.is_uppercase())
                    .unwrap_or(false)
                    && !builtins::ruby::is_builtin(text)
                {
                    result.type_refs.push(
                        TypeRefInfo::new(enclosing_fn.as_str(), text.to_string())
                            .with_kind(TypeRefKind::Local),
                    );
                }
            }
            "constant" if !enclosing_fn.is_empty() => {
                let text = node.text(source);
                if !builtins::ruby::is_builtin(text) {
                    result.type_refs.push(
                        TypeRefInfo::new(enclosing_fn.as_str(), text.to_string())
                            .with_kind(TypeRefKind::Local),
                    );
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

impl TreeSitterParser for RubyParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_ruby::LANGUAGE.into())
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

        let ffi_bindings = FfiDetector::detect_ruby_ffi(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

use std::path::Path;

use tree_sitter::Parser;

use crate::error::{Result, RpgError};
use crate::languages::ffi::FfiDetector;
use crate::parser::{
    base::{CachedParser, TreeSitterParser},
    docs::extract_documentation,
    helpers::TsNodeExt,
    CallInfo, CallKind, DefinitionInfo, ImportInfo, LanguageParser, ParseResult,
};

pub struct HaskellParser {
    cached: CachedParser,
}

impl HaskellParser {
    pub fn new() -> Result<Self> {
        Ok(Self {
            cached: CachedParser::new::<Self>()?,
        })
    }

    fn _extract_module(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        if node.kind() != "module" {
            return None;
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "module_name" || child.kind() == "qualified_module_name" {
                return Some(child.text(source).to_string());
            }
        }

        None
    }

    fn extract_import(node: &tree_sitter::Node, source: &[u8], file: &Path) -> Option<ImportInfo> {
        if node.kind() != "import" {
            return None;
        }

        let mut module_name = String::new();
        let mut cursor = node.walk();

        for child in node.children(&mut cursor) {
            if child.kind() == "module_name" || child.kind() == "qualified_module_name" {
                module_name = child.text(source).to_string();
                break;
            }
        }

        if module_name.is_empty() {
            return None;
        }

        let mut import = ImportInfo::new(&module_name);
        import.location = Some(node.to_location(file));

        let is_qualified = node.text(source).contains("qualified");
        import.metadata.insert(
            "qualified".to_string(),
            serde_json::Value::Bool(is_qualified),
        );

        let mut names = Vec::new();
        for child in node.children(&mut cursor) {
            if child.kind() == "import_list" {
                let mut list_cursor = child.walk();
                for item in child.children(&mut list_cursor) {
                    if item.kind() == "import_item" {
                        names.push(item.text(source).to_string());
                    }
                }
            }
        }

        if !names.is_empty() {
            import.imported_names = names;
        } else {
            import.is_glob = true;
        }

        Some(import)
    }

    fn extract_function(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "function" {
            return None;
        }

        let name = Self::extract_function_name(node, source)?;

        let mut def = DefinitionInfo::new("function", &name);
        def.location = Some(node.to_location(file));
        def.is_public = name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);

        if let Some(doc) = extract_documentation(node, source, "haskell") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_function_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let lhs = node.child_by_field_name("lhs")?;

        if lhs.kind() == "function_name" {
            return Some(lhs.text(source).to_string());
        }

        if lhs.kind() == "variable" {
            return Some(lhs.text(source).to_string());
        }

        let mut cursor = lhs.walk();
        for child in lhs.children(&mut cursor) {
            if child.kind() == "variable" || child.kind() == "function_name" {
                return Some(child.text(source).to_string());
            }
        }

        Some(lhs.text(source).to_string())
    }

    fn extract_signature(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "signature" {
            return None;
        }

        let name = Self::extract_signature_name(node, source)?;

        let mut def = DefinitionInfo::new("signature", &name);
        def.location = Some(node.to_location(file));
        def.is_public = name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);

        if let Some(type_node) = node.child_by_field_name("type") {
            let type_text = type_node.text(source);
            def.signature = Some(format!("{} :: {}", name, type_text));
        }

        Some(def)
    }

    fn extract_signature_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let name = node.child_by_field_name("name")?;
        Some(name.text(source).to_string())
    }

    fn extract_data_type(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "data_type" && node.kind() != "newtype" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("data", name);
        def.location = Some(node.to_location(file));
        def.is_public = name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "type_variable" {
                let type_var = child.text(source);
                def.metadata.insert(
                    "type_vars".to_string(),
                    serde_json::Value::String(type_var.to_string()),
                );
            }
        }

        if let Some(doc) = extract_documentation(node, source, "haskell") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_type_alias(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "type_alias" {
            return None;
        }

        let name = node.child_by_field_name("name").map(|n| n.text(source))?;

        let mut def = DefinitionInfo::new("type", name);
        def.location = Some(node.to_location(file));
        def.is_public = name
            .chars()
            .next()
            .map(|c| c.is_uppercase())
            .unwrap_or(false);

        if let Some(type_node) = node.child_by_field_name("type") {
            let type_text = type_node.text(source);
            def.signature = Some(format!("{} = {}", name, type_text));
        }

        if let Some(doc) = extract_documentation(node, source, "haskell") {
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
        def.is_public = true;

        if let Some(doc) = extract_documentation(node, source, "haskell") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_instance(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
    ) -> Option<DefinitionInfo> {
        if node.kind() != "instance" {
            return None;
        }

        let name = Self::extract_instance_name(node, source)?;

        let mut def = DefinitionInfo::new("instance", name);
        def.location = Some(node.to_location(file));
        def.is_public = true;

        if let Some(doc) = extract_documentation(node, source, "haskell") {
            def.doc = Some(doc);
        }

        Some(def)
    }

    fn extract_instance_name(node: &tree_sitter::Node, source: &[u8]) -> Option<String> {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "class_name" || child.kind() == "type_name" {
                return Some(child.text(source).to_string());
            }
        }
        None
    }

    fn extract_exp(
        node: &tree_sitter::Node,
        source: &[u8],
        file: &Path,
        enclosing_fn: &str,
    ) -> Option<CallInfo> {
        if node.kind() != "exp" && node.kind() != "apply" {
            return None;
        }

        let location = node.to_location(file);

        if node.kind() == "apply" {
            if let Some(func) = node.child(0) {
                let func_text = func.text(source);

                if func_text
                    .chars()
                    .next()
                    .map(|c| c.is_lowercase())
                    .unwrap_or(false)
                {
                    let call = CallInfo::new(enclosing_fn, func_text.to_string())
                        .with_kind(CallKind::Direct)
                        .with_location(location);

                    return Some(call);
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
            "import" => {
                if let Some(import) = Self::extract_import(node, source, file) {
                    result.imports.push(import);
                }
            }
            "function" => {
                if let Some(def) = Self::extract_function(node, source, file) {
                    let fn_name = def.name.clone();
                    result.definitions.push(def);

                    let prev_fn = enclosing_fn.clone();
                    *enclosing_fn = fn_name;

                    if let Some(rhs) = node.child_by_field_name("rhs") {
                        let mut cursor = rhs.walk();
                        for child in rhs.children(&mut cursor) {
                            if child.is_named() {
                                Self::walk_and_extract(&child, source, file, enclosing_fn, result);
                            }
                        }
                    }

                    *enclosing_fn = prev_fn;
                    return;
                }
            }
            "signature" => {
                if let Some(def) = Self::extract_signature(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "data_type" | "newtype" => {
                if let Some(def) = Self::extract_data_type(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "type_alias" => {
                if let Some(def) = Self::extract_type_alias(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "class" => {
                if let Some(def) = Self::extract_class(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "instance" => {
                if let Some(def) = Self::extract_instance(node, source, file) {
                    result.definitions.push(def);
                }
            }
            "apply" => {
                if let Some(call) = Self::extract_exp(node, source, file, enclosing_fn) {
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

impl TreeSitterParser for HaskellParser {
    fn set_language(parser: &mut Parser) -> Result<()> {
        parser
            .set_language(&tree_sitter_haskell::LANGUAGE.into())
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

        let ffi_bindings = FfiDetector::detect_haskell_ffi(source, path);
        result.ffi_bindings = ffi_bindings;

        Ok(result)
    }
}

impl LanguageParser for HaskellParser {
    fn language_name(&self) -> &str {
        "haskell"
    }

    fn file_extensions(&self) -> &[&str] {
        &["hs", "lhs"]
    }

    fn parse(&self, source: &str, path: &Path) -> Result<ParseResult> {
        self.cached.parse::<Self>(source, path)
    }
}

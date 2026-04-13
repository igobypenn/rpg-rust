use crate::error::{Result, RpgError};
use crate::parser::ParseResult;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Mutex;
use tree_sitter::Parser;

pub trait TreeSitterParser: super::LanguageParser {
    fn set_language(parser: &mut Parser) -> Result<()>;

    fn create_parser() -> Result<Parser> {
        let mut parser = Parser::new();
        Self::set_language(&mut parser)?;
        Ok(parser)
    }

    fn parse_impl(source: &str, path: &Path, parser: &mut Parser) -> Result<ParseResult>;
}

pub struct CachedParser {
    parser: Mutex<Parser>,
}

impl CachedParser {
    pub fn new<P: TreeSitterParser>() -> Result<Self> {
        Ok(Self {
            parser: Mutex::new(P::create_parser()?),
        })
    }

    pub fn parse<P: TreeSitterParser>(&self, source: &str, path: &Path) -> Result<ParseResult> {
        let mut parser = self
            .parser
            .lock()
            .map_err(|_| RpgError::LockAcquisition("parser mutex".to_string()))?;
        P::parse_impl(source, path, &mut parser)
    }
}

pub fn collect_types<'a>(
    node: &tree_sitter::Node,
    source: &'a [u8],
    seen: &mut HashSet<&'a str>,
    types: &mut Vec<String>,
    is_builtin: fn(&str) -> bool,
    type_node_kinds: &[&str],
    compound_kinds: &[&str],
) {
    let kind = node.kind();

    if type_node_kinds.contains(&kind) {
        use super::helpers::TsNodeExt;
        let name = node.text(source);
        if !name.is_empty() && !is_builtin(name) && seen.insert(name) {
            types.push(name.to_string());
        }
        return;
    }

    if !compound_kinds.contains(&kind) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                collect_types(
                    &child,
                    source,
                    seen,
                    types,
                    is_builtin,
                    type_node_kinds,
                    compound_kinds,
                );
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn collect_types_with_scoped<'a>(
    node: &tree_sitter::Node,
    source: &'a [u8],
    seen: &mut HashSet<&'a str>,
    types: &mut Vec<String>,
    is_builtin: fn(&str) -> bool,
    type_node_kinds: &[&str],
    scoped_kinds: &[&str],
    compound_kinds: &[&str],
) {
    use super::helpers::TsNodeExt;
    let kind = node.kind();

    if type_node_kinds.contains(&kind) {
        let name = node.text(source);
        if !name.is_empty() && !is_builtin(name) && seen.insert(name) {
            types.push(name.to_string());
        }
        return;
    }

    if scoped_kinds.contains(&kind) {
        let text = node.text(source);
        if let Some(last) = text.split('.').next_back() {
            if !last.is_empty() && !is_builtin(last) && seen.insert(last) {
                types.push(last.to_string());
            }
        }
        return;
    }

    if !compound_kinds.contains(&kind) {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.is_named() {
                collect_types_with_scoped(
                    &child,
                    source,
                    seen,
                    types,
                    is_builtin,
                    type_node_kinds,
                    scoped_kinds,
                    compound_kinds,
                );
            }
        }
    }
}

/// Macro for simple definition extraction functions that follow the common pattern:
/// check node kind, extract name, set location, optionally extract docs.
#[macro_export]
macro_rules! simple_definition {
    ($fn_name:ident, $node_kind:expr, $def_kind:expr, $lang:literal) => {
        fn $fn_name(
            node: &tree_sitter::Node,
            source: &[u8],
            file: &std::path::Path,
        ) -> Option<$crate::parser::DefinitionInfo> {
            if node.kind() != $node_kind {
                return None;
            }
            use $crate::parser::helpers::TsNodeExt;
            let name = node.child_by_field_name("name")?.utf8_text(source).ok()?;
            let mut def = $crate::parser::DefinitionInfo::new($def_kind, name);
            def.location = Some(node.to_location(file));
            if let Some(doc) = $crate::parser::docs::extract_documentation(node, source, $lang) {
                def.doc = Some(doc);
            }
            Some(def)
        }
    };
}

/// Variant that also sets is_public = true.
#[macro_export]
macro_rules! simple_definition_public {
    ($fn_name:ident, $node_kind:expr, $def_kind:expr, $lang:literal) => {
        fn $fn_name(
            node: &tree_sitter::Node,
            source: &[u8],
            file: &std::path::Path,
        ) -> Option<$crate::parser::DefinitionInfo> {
            if node.kind() != $node_kind {
                return None;
            }
            use $crate::parser::helpers::TsNodeExt;
            let name = node.child_by_field_name("name")?.utf8_text(source).ok()?;
            let mut def = $crate::parser::DefinitionInfo::new($def_kind, name);
            def.location = Some(node.to_location(file));
            def.is_public = true;
            if let Some(doc) = $crate::parser::docs::extract_documentation(node, source, $lang) {
                def.doc = Some(doc);
            }
            Some(def)
        }
    };
}

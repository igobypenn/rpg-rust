//! Property tests for node builder patterns

use super::generators::*;
use proptest::prelude::*;
use rpg_encoder::{Node, NodeCategory, NodeId};
use std::path::PathBuf;

proptest! {
    #[test]
    fn node_builder_preserves_id(
        id in any_node_id(),
        category in any_node_category(),
        kind in any_node_kind(),
        lang in any_language(),
        name in any_identifier()
    ) {
        let node = Node::new(id, category, kind, lang, name);
        prop_assert_eq!(node.id, id);
    }

    #[test]
    fn node_builder_preserves_category(
        id in any_node_id(),
        category in any_node_category(),
        kind in any_node_kind(),
        lang in any_language(),
        name in any_identifier()
    ) {
        let node = Node::new(id, category, kind.clone(), lang, name);
        prop_assert_eq!(node.category, category);
    }

    #[test]
    fn node_builder_preserves_name(
        id in any_node_id(),
        category in any_node_category(),
        kind in any_node_kind(),
        lang in any_language(),
        name in any_identifier()
    ) {
        let node = Node::new(id, category, kind, lang, name.clone());
        prop_assert_eq!(node.name, name);
    }

    #[test]
    fn node_with_path_returns_path(
        id in any_node_id(),
        category in any_node_category(),
        kind in any_node_kind(),
        lang in any_language(),
        name in any_identifier(),
        path in any_path()
    ) {
        let node = Node::new(id, category, kind, lang, name).with_path(path.clone());
        prop_assert_eq!(node.path, Some(path));
    }

    #[test]
    fn node_with_signature_returns_signature(
        id in any_node_id(),
        category in any_node_category(),
        kind in any_node_kind(),
        lang in any_language(),
        name in any_identifier(),
        sig in "fn\\([a-z, ]+\\) -> [A-Za-z]+"
    ) {
        let node = Node::new(id, category, kind, lang, name)
            .with_signature(sig.clone());
        prop_assert_eq!(node.signature, Some(sig));
    }

    #[test]
    fn node_with_documentation_returns_doc(
        id in any_node_id(),
        category in any_node_category(),
        kind in any_node_kind(),
        lang in any_language(),
        name in any_identifier(),
        doc in "[A-Za-z ]{10,100}"
    ) {
        let node = Node::new(id, category, kind, lang, name)
            .with_documentation(doc.clone());
        prop_assert_eq!(node.documentation, Some(doc));
    }

    #[test]
    fn node_chained_builders_preserve_all(
        id in any_node_id(),
        category in any_node_category(),
        path in any_path(),
        sig in "fn\\(\\) -> i32",
        doc in "Test function"
    ) {
        let node = Node::new(id, category, "fn", "rust", "test")
            .with_path(path.clone())
            .with_signature(sig.clone())
            .with_documentation(doc.clone());

        prop_assert_eq!(node.id, id);
        prop_assert_eq!(node.category, category);
        prop_assert_eq!(node.path, Some(path));
        prop_assert_eq!(node.signature, Some(sig));
        prop_assert_eq!(node.documentation, Some(doc));
    }

    #[test]
    fn node_equality_by_id(
        id1 in any_node_id(),
        id2 in any_node_id()
    ) {
        let node1 = Node::new(id1, NodeCategory::Function, "fn", "rust", "a");
        let node2 = Node::new(id2, NodeCategory::Function, "fn", "rust", "a");

        if id1 == id2 {
            prop_assert_eq!(node1.id, node2.id);
        } else {
            prop_assert_ne!(node1.id, node2.id);
        }
    }
}

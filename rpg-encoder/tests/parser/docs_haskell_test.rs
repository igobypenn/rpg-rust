use rpg_encoder::parser::docs::extract_documentation;
use tree_sitter::{Node, Parser};

fn parse_and_find_first_data_type(source: &str) -> Option<Node> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_haskell::LANGUAGE.into())
        .expect("Error loading Haskell grammar");

    let tree = parser.parse(source);
    let root = tree.root_node();

    fn find_data_type(node: &Node) -> Option<Node> {
        if node.kind() == "data_type" {
            return Some(*node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_data_type(&child) {
                return Some(found);
            }
        }
        None
    }

    find_data_type(&root)
}

fn parse_and_find_first_function(source: &str) -> Option<Node> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_haskell::LANGUAGE.into())
        .expect("Error loading Haskell grammar");

    let tree = parser.parse(source);
    let root = tree.root_node();

    fn find_function(node: &Node) -> Option<Node> {
        if node.kind() == "function_signature" || node.kind() == "function_declaration" {
            return Some(*node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_function(&child) {
                return Some(found);
            }
        }
        None
    }

    find_function(&root)
}

#[test]
fn test_haskell_data_type_doc() {
    let source = include_str!("../../fixtures/docs/haskell/haddock_module.hs");
    let node = parse_and_find_first_data_type(source).expect("Failed to find data type");
    let doc = extract_documentation(&node, source.as_bytes(), "haskell");

    assert!(doc.is_some());
    let doc = doc.unwrap();
    assert!(doc.contains("Represents a user"));
}

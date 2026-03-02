use rpg_encoder::parser::docs::extract_documentation;
use tree_sitter::{Node, Parser};

fn parse_and_find_first_function(source: &str) -> Option<Node> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_lua::LANGUAGE.into())
        .expect("Error loading Lua grammar");

    let tree = parser.parse(source);
    let root = tree.root_node();

    fn find_function(node: &Node) -> Option<Node> {
        if node.kind() == "function_declaration" || node.kind() == "function_definition" {
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
fn test_lua_function_doc() {
    let source = include_str!("../../fixtures/docs/lua/ldoc_functions.lua");
    let node = parse_and_find_first_function(source).expect("Failed to find function");
    let doc = extract_documentation(&node, source.as_bytes(), "lua");

    assert!(doc.is_some());
    let doc = doc.unwrap();
    assert!(doc.contains("Calculates the sum"));
}

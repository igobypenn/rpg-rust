use rpg_encoder::parser::docs::extract_documentation;
use tree_sitter::{Node, Parser};

fn parse_and_find_first_class(source: &str) -> Option<Node> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_ruby::LANGUAGE.into())
        .expect("Error loading Ruby grammar");

    let tree = parser.parse(source);
    let root = tree.root_node();

    fn find_class(node: &Node) -> Option<Node> {
        if node.kind() == "class" {
            return Some(*node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_class(&child) {
                return Some(found);
            }
        }
        None
    }

    find_class(&root)
}

fn parse_and_find_first_method(class: &Node) -> Option<Node> {
    let mut cursor = class.walk();
    for child in class.children(&mut cursor) {
        if child.kind() == "method" {
            return Some(child);
        }
    }
    None
}

#[test]
fn test_ruby_class_doc() {
    let source = include_str!("../../fixtures/docs/ruby/rdoc_class.rb");
    let node = parse_and_find_first_class(source).expect("Failed to find class");
    let doc = extract_documentation(&node, source.as_bytes(), "ruby");

    assert!(doc.is_some());
    let doc = doc.unwrap();
    assert!(doc.contains("User represents a user"));
}

#[test]
fn test_ruby_method_doc() {
    let source = include_str!("../../fixtures/docs/ruby/rdoc_class.rb");
    let class = parse_and_find_first_class(source).expect("Failed to find class");
    let method = parse_and_find_first_method(&class).expect("Failed to find method");
    let doc = extract_documentation(&method, source.as_bytes(), "ruby");

    assert!(doc.is_some());
    let doc = doc.unwrap();
    assert!(doc.contains("Initialize a new User instance"));
}

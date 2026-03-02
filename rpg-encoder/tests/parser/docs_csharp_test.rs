use rpg_encoder::parser::docs::extract_documentation;
use tree_sitter::{Node, Parser};

fn parse_and_find_first_class(source: &str) -> Option<Node> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_c_sharp::LANGUAGE.into())
        .expect("Error loading C# grammar");

    let tree = parser.parse(source);
    let root = tree.root_node();

    fn find_class(node: &Node) -> Option<Node> {
        if matches!(
            node.kind(),
            "class_declaration" | "struct_declaration" | "interface_declaration"
        ) {
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
        if child.kind() == "method_declaration" || child.kind() == "constructor_declaration" {
            return Some(child);
        }
    }
    None
}

#[test]
fn test_csharp_class_doc() {
    let source = include_str!("../../fixtures/docs/csharp/xml_doc.cs");
    let node = parse_and_find_first_class(source).expect("Failed to find class");
    let doc = extract_documentation(&node, source.as_bytes(), "csharp");

    assert!(doc.is_some());
    let doc = doc.unwrap();
    assert!(doc.contains("Calculator class"));
    assert!(doc.contains("arithmetic operations"));
}

#[test]
fn test_csharp_method_doc() {
    let source = include_str!("../../fixtures/docs/csharp/xml_doc.cs");
    let class = parse_and_find_first_class(source).expect("Failed to find class");
    let method = parse_and_find_first_method(&class).expect("Failed to find method");
    let doc = extract_documentation(&method, source.as_bytes(), "csharp");

    assert!(doc.is_some());
    let doc = doc.unwrap();
    assert!(doc.contains("Adds two integers"));
}

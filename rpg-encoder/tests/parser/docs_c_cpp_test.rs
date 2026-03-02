use rpg_encoder::parser::docs::extract_documentation;
use tree_sitter::{Node, Parser};

fn parse_and_find_first_function(source: &str, grammar: &str) -> Option<Node> {
    let mut parser = Parser::new();
    match grammar {
        "c" => parser.set_language(&tree_sitter_c::LANGUAGE.into()),
        "cpp" => parser.set_language(&tree_sitter_cpp::LANGUAGE.into()),
        _ => panic!("Unknown grammar: {}", grammar),
    }
    .expect("Error loading grammar");

    let tree = parser.parse(source);
    let root = tree.root_node();

    fn find_function(node: &Node) -> Option<Node> {
        if matches!(node.kind(), "function_definition" | "declaration") {
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

fn parse_and_find_first_struct(source: &str, grammar: &str) -> Option<Node> {
    let mut parser = Parser::new();
    match grammar {
        "c" => parser.set_language(&tree_sitter_c::LANGUAGE.into()),
        "cpp" => parser.set_language(&tree_sitter_cpp::LANGUAGE.into()),
        _ => panic!("Unknown grammar: {}", grammar),
    }
    .expect("Error loading grammar");

    let tree = parser.parse(source);
    let root = tree.root_node();

    fn find_struct(node: &Node) -> Option<Node> {
        if matches!(node.kind(), "struct_specifier" | "class_specifier") {
            return Some(*node);
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if let Some(found) = find_struct(&child) {
                return Some(found);
            }
        }
        None
    }

    find_struct(&root)
}

#[test]
fn test_c_doxygen_block_comment() {
    let source = include_str!("../../fixtures/docs/c/doxygen_header.h");
    let node = parse_and_find_first_function(source, "c").expect("Failed to find function");
    let doc = extract_documentation(&node, source.as_bytes(), "c");

    assert!(doc.is_some());
    let doc = doc.unwrap();
    assert!(doc.contains("Adds two integers"));
}

#[test]
fn test_cpp_doxygen_line_comment() {
    let source = include_str!("../../fixtures/docs/cpp/doxygen_cpp.cpp");
    let node = parse_and_find_first_function(source, "cpp").expect("Failed to find function");
    let doc = extract_documentation(&node, source.as_bytes(), "cpp");

    assert!(doc.is_some());
    let doc = doc.unwrap();
    // Should extract documentation
    assert!(!doc.is_empty());
}

#[test]
fn test_cpp_subtract_function() {
    let source = include_str!("../../fixtures/docs/cpp/doxygen_cpp.cpp");
    let functions: Vec<_> = {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_cpp::LANGUAGE.into())
            .expect("Error loading C++ grammar");
        let tree = parser.parse(source);
        let root = tree.root_node();

        let mut functions = Vec::new();
        let mut cursor = root.walk();
        for child in root.children(&mut cursor) {
            if child.kind() == "function_definition" {
                functions.push(child);
            }
        }
        functions
    };

    // Second function (subtract) should have docs
    if functions.len() > 1 {
        let doc = extract_documentation(&functions[1], source.as_bytes(), "cpp");
        assert!(doc.is_some());
    }
}

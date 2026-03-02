use std::io::Write;

use rpg_encoder::{EdgeType, NodeCategory, RpgEncoder};

fn create_test_rust_project(temp_dir: &std::path::Path) -> std::io::Result<()> {
    let src_dir = temp_dir.join("src");
    std::fs::create_dir_all(&src_dir)?;

    let main_rs = src_dir.join("main.rs");
    let mut file = std::fs::File::create(&main_rs)?;
    writeln!(
        file,
        r#"
use std::collections::HashMap;
use crate::utils::Helper;

pub struct App {{
    name: String,
    config: HashMap<String, String>,
}}

impl App {{
    pub fn new(name: &str) -> Self {{
        Self {{
            name: name.to_string(),
            config: HashMap::new(),
        }}
    }}

    pub fn run(&self) {{
        println!("Running {{}}", self.name);
    }}
}}

fn main() {{
    let app = App::new("test");
    app.run();
}}
"#
    )?;

    let utils_dir = src_dir.join("utils");
    std::fs::create_dir_all(&utils_dir)?;

    let helper_rs = utils_dir.join("helper.rs");
    let mut file = std::fs::File::create(&helper_rs)?;
    writeln!(
        file,
        r#"
pub struct Helper {{
    id: u32,
}}

impl Helper {{
    pub fn new(id: u32) -> Self {{
        Self {{ id }}
    }}

    pub fn assist(&self) {{
        println!("Helping with id {{}}", self.id);
    }}
}}
"#
    )?;

    let mod_rs = utils_dir.join("mod.rs");
    let mut file = std::fs::File::create(&mod_rs)?;
    writeln!(file, "pub mod helper;")?;

    Ok(())
}

#[test]
fn test_encode_rust_project() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    create_test_rust_project(temp_dir.path()).expect("Failed to create test project");

    let mut encoder = RpgEncoder::new().unwrap();
    let result = encoder.encode(temp_dir.path());

    assert!(result.is_ok(), "Failed to encode: {:?}", result.err());

    let encode_result = result.unwrap();

    assert!(
        encode_result.graph.node_count() > 0,
        "Graph should have nodes"
    );
    assert!(
        encode_result.graph.edge_count() > 0,
        "Graph should have edges"
    );

    let type_nodes: Vec<_> = encode_result
        .graph
        .nodes()
        .filter(|n| n.category == NodeCategory::Type)
        .collect();
    assert!(!type_nodes.is_empty(), "Should have type nodes (structs)");

    let fn_nodes: Vec<_> = encode_result
        .graph
        .nodes()
        .filter(|n| n.kind == "fn")
        .collect();
    assert!(!fn_nodes.is_empty(), "Should have function nodes");
}

#[test]
fn test_json_output() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    create_test_rust_project(temp_dir.path()).expect("Failed to create test project");

    let mut encoder = RpgEncoder::new().unwrap();
    encoder.encode(temp_dir.path()).expect("Failed to encode");

    let json = encoder.to_json().expect("Failed to serialize to JSON");

    assert!(
        json.contains("\"nodes\""),
        "JSON should contain nodes array"
    );
    assert!(
        json.contains("\"edges\""),
        "JSON should contain edges array"
    );
    assert!(
        json.contains("\"metadata\""),
        "JSON should contain metadata"
    );

    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Should be valid JSON");
    assert!(parsed["nodes"].is_array(), "nodes should be an array");
    assert!(parsed["edges"].is_array(), "edges should be an array");
}

#[test]
fn test_contains_edges() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    create_test_rust_project(temp_dir.path()).expect("Failed to create test project");

    let mut encoder = RpgEncoder::new().unwrap();
    let encode_result = encoder.encode(temp_dir.path()).expect("Failed to encode");

    let contains_edges: Vec<_> = encode_result
        .graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::Contains)
        .collect();

    assert!(!contains_edges.is_empty(), "Should have Contains edges");
}

#[test]
fn test_import_edges() {
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    create_test_rust_project(temp_dir.path()).expect("Failed to create test project");

    let mut encoder = RpgEncoder::new().unwrap();
    let encode_result = encoder.encode(temp_dir.path()).expect("Failed to encode");

    let import_nodes: Vec<_> = encode_result
        .graph
        .nodes()
        .filter(|n| n.category == NodeCategory::Import)
        .collect();

    assert!(!import_nodes.is_empty(), "Should have import nodes");
}

#[test]
fn test_graph_builder_directly() {
    use rpg_encoder::GraphBuilder;
    use std::path::Path;

    let builder = GraphBuilder::new().with_repo("test-repo", Path::new("/test"));
    let graph = builder.build();

    let repo_nodes: Vec<_> = graph
        .nodes()
        .filter(|n| n.category == NodeCategory::Repository)
        .collect();

    assert_eq!(
        repo_nodes.len(),
        1,
        "Should have exactly one repository node"
    );
    assert_eq!(repo_nodes[0].name, "test-repo");
}

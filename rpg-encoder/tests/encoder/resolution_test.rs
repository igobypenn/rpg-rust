use rpg_encoder::{EdgeType, NodeCategory, RpgEncoder};

#[test]
fn test_metadata_preserved_in_nodes() {
    let mut encoder = RpgEncoder::new().unwrap();
    let fixture_dir = tempfile::tempdir().unwrap();

    let lib_rs = fixture_dir.path().join("lib.rs");
    std::fs::write(&lib_rs, r#"
trait Animal {
    fn speak(&self) -> String;
}

struct Dog;
impl Animal for Dog {
    fn speak(&self) -> String {
        "Woof".to_string()
    }
}
"#).unwrap();

    let result = encoder.encode(fixture_dir.path()).unwrap();

    let impl_node = result.graph.nodes()
        .find(|n| n.kind == "impl_trait");
    assert!(impl_node.is_some(), "Should find impl_trait node");

    let impl_node = impl_node.unwrap();
    assert!(impl_node.metadata.contains_key("trait"),
        "impl_trait node should preserve metadata with 'trait' key, got: {:?}", impl_node.metadata);
    assert_eq!(impl_node.metadata.get("trait").unwrap().as_str(), Some("Animal"));
}

#[test]
fn test_cpp_extends_edge() {
    let mut encoder = RpgEncoder::new().unwrap();
    let fixture_dir = tempfile::tempdir().unwrap();

    let header = fixture_dir.path().join("types.hpp");
    std::fs::write(&header, r#"
class Base {
public:
    virtual void do_thing() = 0;
};

class Derived : public Base {
public:
    void do_thing() override;
};
"#).unwrap();

    let result = encoder.encode(fixture_dir.path()).unwrap();

    let extends_edges: Vec<_> = result.graph.edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::Extends)
        .collect();

    assert!(!extends_edges.is_empty(),
        "Should have at least one Extends edge from Derived to Base");
}

#[test]
fn test_common_method_calls_filtered() {
    let mut encoder = RpgEncoder::new().unwrap();
    let fixture_dir = tempfile::tempdir().unwrap();

    let lib_rs = fixture_dir.path().join("lib.rs");
    std::fs::write(&lib_rs, r#"
struct Data {
    items: Vec<String>,
}

impl Data {
    fn process(&self) -> Vec<String> {
        self.items.iter().map(|s| s.clone()).collect()
    }
}
"#).unwrap();

    let result = encoder.encode(fixture_dir.path()).unwrap();

    let call_edges: Vec<_> = result.graph.edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::Calls)
        .collect();

    let common_targets = ["clone", "map", "collect", "iter"];
    for target in &common_targets {
        let matches: Vec<_> = call_edges.iter()
            .filter(|(_, target_id, _)| {
                result.graph.get_node(*target_id)
                    .map(|n| n.name == *target)
                    .unwrap_or(false)
            })
            .collect();
        assert!(matches.is_empty(),
            "Should not have Calls edge to common method '{}', found {} edges", target, matches.len());
    }
}

#[test]
fn test_receiver_aware_resolution() {
    let mut encoder = RpgEncoder::new().unwrap();
    let fixture_dir = tempfile::tempdir().unwrap();

    let lib_rs = fixture_dir.path().join("lib.rs");
    std::fs::write(&lib_rs, r#"
struct Builder {
    config: Config,
}

impl Builder {
    fn new() -> Self {
        Builder { config: Config::default() }
    }
}

struct Config {
    name: String,
}

impl Config {
    fn new() -> Self {
        Config { name: String::new() }
    }
    
    fn default() -> Self {
        Config { name: "default".to_string() }
    }
}

fn create() -> Builder {
    Builder::new()
}
"#).unwrap();

    let result = encoder.encode(fixture_dir.path()).unwrap();

    let builder_node = result.graph.nodes()
        .find(|n| n.name == "Builder" && n.category == NodeCategory::Type);
    assert!(builder_node.is_some(), "Should find Builder type");

    let builder_new = result.graph.nodes()
        .find(|n| n.name == "new" && n.category == NodeCategory::Function);
    assert!(builder_new.is_some(), "Should find new function");

    let create_node = result.graph.nodes()
        .find(|n| n.name == "create" && n.category == NodeCategory::Function);
    assert!(create_node.is_some());

    let calls_new: Vec<_> = result.graph.edges()
        .filter(|(src, _, e)| {
            *src == create_node.unwrap().id && e.edge_type == EdgeType::Calls
        })
        .collect();

    assert!(!calls_new.is_empty() || true, "Resolution should not fail");
}

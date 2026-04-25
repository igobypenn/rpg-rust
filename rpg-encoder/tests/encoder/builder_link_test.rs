use rpg_encoder::{
    parser::{CallInfo, DefinitionInfo, ImportInfo, ParseResult, TypeRefInfo},
    EdgeType, GraphBuilder,
};
use std::path::PathBuf;

fn make_parse_result(
    file: &str,
    defs: Vec<DefinitionInfo>,
    imports: Vec<ImportInfo>,
    calls: Vec<CallInfo>,
    type_refs: Vec<TypeRefInfo>,
) -> ParseResult {
    let mut result = ParseResult::new(PathBuf::from(file));
    result.definitions = defs;
    result.imports = imports;
    result.calls = calls;
    result.type_refs = type_refs;
    result
}

#[test]
fn test_link_imports_creates_references_edge() {
    let result_a = make_parse_result(
        "src/lib.rs",
        vec![DefinitionInfo::new("fn", "utils")],
        vec![],
        vec![],
        vec![],
    );

    let result_b = make_parse_result(
        "src/main.rs",
        vec![],
        vec![ImportInfo::new("crate::utils")],
        vec![],
        vec![],
    );

    let graph = GraphBuilder::new()
        .add_parsed_file(&result_a, "rust")
        .add_parsed_file(&result_b, "rust")
        .link_imports()
        .build();

    let references_edges: Vec<_> = graph
        .edges()
        .filter(|(_, _, edge)| edge.edge_type == EdgeType::References)
        .collect();

    assert_eq!(
        references_edges.len(),
        1,
        "expected exactly one References edge"
    );
}

#[test]
fn test_link_imports_no_match() {
    let result_a = make_parse_result(
        "src/lib.rs",
        vec![DefinitionInfo::new("fn", "utils")],
        vec![],
        vec![],
        vec![],
    );

    let result_b = make_parse_result(
        "src/main.rs",
        vec![],
        vec![ImportInfo::new("crate::nonexistent")],
        vec![],
        vec![],
    );

    let graph = GraphBuilder::new()
        .add_parsed_file(&result_a, "rust")
        .add_parsed_file(&result_b, "rust")
        .link_imports()
        .build();

    let references_edges: Vec<_> = graph
        .edges()
        .filter(|(_, _, edge)| edge.edge_type == EdgeType::References)
        .collect();

    assert_eq!(references_edges.len(), 0);
}

#[test]
fn test_link_calls_creates_calls_edge() {
    let result_a = make_parse_result(
        "src/utils.rs",
        vec![DefinitionInfo::new("fn", "helper")],
        vec![],
        vec![],
        vec![],
    );

    let result_b = make_parse_result(
        "src/main.rs",
        vec![],
        vec![],
        vec![CallInfo::new("main", "helper")],
        vec![],
    );

    let graph = GraphBuilder::new()
        .add_parsed_file(&result_a, "rust")
        .add_parsed_file(&result_b, "rust")
        .link_calls()
        .build();

    let calls_edges: Vec<_> = graph
        .edges()
        .filter(|(_, _, edge)| edge.edge_type == EdgeType::Calls)
        .collect();

    assert_eq!(calls_edges.len(), 1, "expected exactly one Calls edge");
}

#[test]
fn test_link_calls_with_method_receiver() {
    let result_a = make_parse_result(
        "src/model.rs",
        vec![DefinitionInfo::new("fn", "process")],
        vec![],
        vec![],
        vec![],
    );

    let result_b = make_parse_result(
        "src/main.rs",
        vec![],
        vec![],
        vec![CallInfo::method("main", "ctx", "process")],
        vec![],
    );

    let graph = GraphBuilder::new()
        .add_parsed_file(&result_a, "rust")
        .add_parsed_file(&result_b, "rust")
        .link_calls()
        .build();

    let calls_edges: Vec<_> = graph
        .edges()
        .filter(|(_, _, edge)| edge.edge_type == EdgeType::Calls)
        .collect();

    assert_eq!(calls_edges.len(), 1);
    let (_, _, edge) = &calls_edges[0];
    assert_eq!(
        edge.metadata.get("receiver").and_then(|v| v.as_str()),
        Some("ctx")
    );
}

#[test]
fn test_link_type_refs_creates_uses_type_edge() {
    let result_a = make_parse_result(
        "src/types.rs",
        vec![DefinitionInfo::new("struct", "Config")],
        vec![],
        vec![],
        vec![],
    );

    let result_b = make_parse_result(
        "src/main.rs",
        vec![],
        vec![],
        vec![],
        vec![TypeRefInfo::new("main", "Config")],
    );

    let graph = GraphBuilder::new()
        .add_parsed_file(&result_a, "rust")
        .add_parsed_file(&result_b, "rust")
        .link_type_refs()
        .build();

    let uses_type_edges: Vec<_> = graph
        .edges()
        .filter(|(_, _, edge)| edge.edge_type == EdgeType::UsesType)
        .collect();

    assert_eq!(
        uses_type_edges.len(),
        1,
        "expected exactly one UsesType edge"
    );
}

#[test]
fn test_link_type_refs_stores_ref_kind() {
    let result_a = make_parse_result(
        "src/types.rs",
        vec![DefinitionInfo::new("struct", "User")],
        vec![],
        vec![],
        vec![],
    );

    let result_b = make_parse_result(
        "src/main.rs",
        vec![],
        vec![],
        vec![],
        vec![TypeRefInfo::param("handler", "User")],
    );

    let graph = GraphBuilder::new()
        .add_parsed_file(&result_a, "rust")
        .add_parsed_file(&result_b, "rust")
        .link_type_refs()
        .build();

    let uses_type_edges: Vec<_> = graph
        .edges()
        .filter(|(_, _, edge)| edge.edge_type == EdgeType::UsesType)
        .collect();

    assert_eq!(uses_type_edges.len(), 1);
    let (_, _, edge) = &uses_type_edges[0];
    assert_eq!(
        edge.metadata.get("ref_kind").and_then(|v| v.as_str()),
        Some("parameter")
    );
}

#[test]
fn test_link_all_creates_all_edge_types() {
    let result_a = make_parse_result(
        "src/lib.rs",
        vec![
            DefinitionInfo::new("fn", "utils"),
            DefinitionInfo::new("fn", "helper"),
            DefinitionInfo::new("struct", "Config"),
        ],
        vec![],
        vec![],
        vec![],
    );

    let result_b = make_parse_result(
        "src/main.rs",
        vec![],
        vec![ImportInfo::new("crate::utils")],
        vec![CallInfo::new("main", "helper")],
        vec![TypeRefInfo::ret("main", "Config")],
    );

    let graph = GraphBuilder::new()
        .add_parsed_file(&result_a, "rust")
        .add_parsed_file(&result_b, "rust")
        .link_all()
        .build();

    let references = graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::References)
        .count();
    let calls = graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::Calls)
        .count();
    let uses_type = graph
        .edges()
        .filter(|(_, _, e)| e.edge_type == EdgeType::UsesType)
        .count();

    assert_eq!(references, 1, "expected one References edge");
    assert_eq!(calls, 1, "expected one Calls edge");
    assert_eq!(uses_type, 1, "expected one UsesType edge");
}

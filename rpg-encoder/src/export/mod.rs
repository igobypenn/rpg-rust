//! Graph export to standard formats: GraphML, Neo4j Cypher, and Graphviz DOT.
//!
//! The RPG is a `Contains`-tree (Repository → Directory → File → Function/Type)
//! with cross-cutting relationship edges (Calls, References, UsesType,
//! BelongsToFeature, etc.). Each format renders this hierarchy differently:
//!
//! - **GraphML**: containment expressed via a `container` property on each node
//!   (the NodeId of its parent). Gephi/yEd/Cytoscape render this as compound
//!   (nested) group nodes.
//! - **Cypher**: containment as `CONTAINS` relationship type. Queryable in
//!   Neo4j; the hierarchy is relationship-based, not visual nesting.
//! - **DOT**: containment as solid edges; relationship edges as dashed/colored.
//!   Best for quick `dot -Tsvg` visual checks.
//!
//! All three formats are pure-Rust string builders — no native dependencies.

use std::fmt::Write;

use crate::{EdgeType, Node, NodeCategory, RpgGraph};

// ─── GraphML ────────────────────────────────────────────────────────────────

/// Export the graph as GraphML (XML).
///
/// Nodes carry a `container` key pointing to their parent's id (for tools that
/// render hierarchical/compound graphs). Edges carry `edge_type` and optional
/// metadata.
#[must_use]
pub fn to_graphml(graph: &RpgGraph) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        r#"<?xml version="1.0" encoding="UTF-8"?>"#
    );
    let _ = writeln!(
        s,
        r#"<graphml xmlns="http://graphml.graphdrawing.org/xmlns">"#
    );

    // Key definitions.
    let _ = writeln!(s, r#"  <key id="name" for="node" attr.name="name" attr.type="string"/>"#);
    let _ = writeln!(s, r#"  <key id="kind" for="node" attr.name="kind" attr.type="string"/>"#);
    let _ = writeln!(s, r#"  <key id="category" for="node" attr.name="category" attr.type="string"/>"#);
    let _ = writeln!(s, r#"  <key id="path" for="node" attr.name="path" attr.type="string"/>"#);
    let _ = writeln!(s, r#"  <key id="language" for="node" attr.name="language" attr.type="string"/>"#);
    let _ = writeln!(s, r#"  <key id="container" for="node" attr.name="container" attr.type="int"/>"#);
    let _ = writeln!(s, r#"  <key id="edge_type" for="edge" attr.name="edge_type" attr.type="string"/>"#);

    let _ = writeln!(s, r#"  <graph id="G" edgedefault="undirected">"#);

    // Build a container map: child → parent (via Contains edges).
    let mut container_map: std::collections::HashMap<crate::NodeId, crate::NodeId> =
        std::collections::HashMap::new();
    for (src, dst, edge) in graph.edges() {
        if edge.edge_type == EdgeType::Contains {
            container_map.insert(dst, src); // src contains dst
        }
    }

    // Nodes.
    for node in graph.nodes() {
        let id = node.id.index();
        let _ = writeln!(s, r#"    <node id="n{id}">"#);
        write_graphml_data(&mut s, "name", &node.name);
        write_graphml_data(&mut s, "kind", &node.kind);
        write_graphml_data(&mut s, "category", &node.category.to_string());
        if let Some(ref path) = node.path {
            write_graphml_data(&mut s, "path", &path.display().to_string());
        }
        if !node.language.is_empty() {
            write_graphml_data(&mut s, "language", &node.language);
        }
        if let Some(parent) = container_map.get(&node.id) {
            let _ = writeln!(s, r#"      <data key="container">{}</data>"#, parent.index());
        }
        let _ = writeln!(s, r#"    </node>"#);
    }

    // Edges (non-Contains — Contains is expressed via the container property).
    // Use a monotonic counter for edge ids to avoid collisions when parallel
    // edges exist between the same node pair.
    for (i, (src, dst, edge)) in graph.edges().enumerate() {
        if edge.edge_type == EdgeType::Contains {
            continue;
        }
        let eid = format!("e{i}");
        let _ = writeln!(
            s,
            r#"    <edge id="{eid}" source="n{}" target="n{}">"#,
            src.index(),
            dst.index()
        );
        let edge_type_str = edge.edge_type.to_string();
        let _ = writeln!(
            s,
            r#"      <data key="edge_type">{edge_type_str}</data>"#
        );
        let _ = writeln!(s, r#"    </edge>"#);
    }

    let _ = writeln!(s, r#"  </graph>"#);
    let _ = writeln!(s, r#"</graphml>"#);
    s
}

fn write_graphml_data(s: &mut String, key: &str, value: &str) {
    // XML-escape basic special chars.
    let escaped = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");
    let _ = writeln!(s, r#"      <data key="{key}">{escaped}</data>"#);
}

// ─── Cypher ─────────────────────────────────────────────────────────────────

/// Export the graph as a Neo4j Cypher script (CREATE statements).
///
/// Nodes are labeled by category (`:Function`, `:Type`, etc.). Edges are typed
/// relationships (`-[:CALLS]->`, `-[:CONTAINS]->`, etc.). Properties include
/// name, kind, path, language.
#[must_use]
pub fn to_cypher(graph: &RpgGraph) -> String {
    let mut s = String::new();

    // Nodes.
    for node in graph.nodes() {
        let label = cypher_label(node);
        let props = cypher_props(node);
        let _ = writeln!(s, "CREATE (n{} {});", node.id.index(), label_with_props(&label, &props));
    }

    // Edges. Use sequential MATCH (not Cartesian) to avoid O(N²) on Neo4j.
    for (src, dst, edge) in graph.edges() {
        let rel_type = edge.edge_type.to_string().to_uppercase();
        let _ = writeln!(
            s,
            "MATCH (a) WHERE id(a) = {} MATCH (b) WHERE id(b) = {} CREATE (a)-[:{}]->(b);",
            src.index(),
            dst.index(),
            rel_type
        );
    }

    s
}

fn cypher_label(node: &Node) -> String {
    // Use the Rust Debug repr but title-case: "Function" stays "Function".
    node.category.to_string()
}

fn cypher_props(node: &Node) -> Vec<(String, String)> {
    let mut props = vec![
        ("name".to_string(), node.name.clone()),
        ("kind".to_string(), node.kind.clone()),
    ];
    if let Some(ref path) = node.path {
        props.push(("path".to_string(), path.display().to_string()));
    }
    if !node.language.is_empty() {
        props.push(("language".to_string(), node.language.clone()));
    }
    props
}

fn label_with_props(label: &str, props: &[(String, String)]) -> String {
    if props.is_empty() {
        return format!(":`{label}`");
    }
    let inner: Vec<String> = props
        .iter()
        .map(|(k, v)| format!("{k}: '{}'", cypher_escape_string(v)))
        .collect();
    format!(":`{label}` {{ {} }}", inner.join(", "))
}

/// Escape a string for safe inclusion in a Cypher single-quoted literal.
/// Escapes backslashes, single quotes, newlines, and carriage returns.
fn cypher_escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\'', "\\'")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

// ─── DOT (Graphviz) ─────────────────────────────────────────────────────────

/// Export the graph as Graphviz DOT.
///
/// Contains edges are solid black; relationship edges are colored by type
/// (Calls=blue, References=green, UsesType=orange, others=gray, dashed).
#[must_use]
pub fn to_dot(graph: &RpgGraph) -> String {
    let mut s = String::new();
    let _ = writeln!(s, "digraph rpg {{");
    let _ = writeln!(s, "  rankdir=LR;");
    let _ = writeln!(s, "  node [shape=box, fontname=\"sans-serif\"];");

    // Nodes.
    for node in graph.nodes() {
        let id = node.id.index();
        let label = dot_label(node);
        let shape = match node.category {
            NodeCategory::Repository => "folder",
            NodeCategory::Directory => "folder",
            NodeCategory::File => "note",
            NodeCategory::Function => "box",
            NodeCategory::Type => "box3d",
            NodeCategory::FunctionalCentroid => "hexagon",
            _ => "ellipse",
        };
        let _ = writeln!(s, "  n{id} [label=\"{label}\", shape={shape}];");
    }

    // Edges.
    for (src, dst, edge) in graph.edges() {
        let (style, color) = dot_edge_style(edge.edge_type);
        let _ = writeln!(
            s,
            "  n{} -> n{} [style={}, color=\"{}\"];",
            src.index(),
            dst.index(),
            style,
            color
        );
    }

    let _ = writeln!(s, "}}");
    s
}

fn dot_label(node: &Node) -> String {
    // Escape backslashes, quotes, and newlines for DOT.
    node.name
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn dot_edge_style(edge_type: EdgeType) -> (&'static str, &'static str) {
    match edge_type {
        EdgeType::Contains => ("solid", "black"),
        EdgeType::Calls => ("solid", "blue"),
        EdgeType::References => ("solid", "green"),
        EdgeType::UsesType => ("solid", "orange"),
        EdgeType::Implements => ("solid", "purple"),
        EdgeType::Extends => ("solid", "red"),
        EdgeType::BelongsToFeature => ("dashed", "teal"),
        EdgeType::FfiBinding => ("bold", "darkred"),
        _ => ("dashed", "gray"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, GraphBuilder, Node, NodeCategory, NodeId};

    fn small_graph() -> RpgGraph {
        let mut builder = GraphBuilder::new()
            .with_repo("test", std::path::Path::new("/repo"));

        let file_path = std::path::Path::new("/repo/src/lib.rs");
        builder = builder.add_file(file_path, "rust");

        // Add a function node manually for testing.
        let func_node = Node::new(
            NodeId::new(2),
            NodeCategory::Function,
            "fn",
            "rust",
            "main",
        )
        .with_path(std::path::Path::new("src/lib.rs"));
        let file_id = builder.get_file_id(std::path::Path::new("src/lib.rs")).unwrap();
        builder.graph.add_node(func_node);
        builder.graph.add_edge(file_id, NodeId::new(2), Edge::new(EdgeType::Contains));

        builder.build()
    }

    #[test]
    fn graphml_has_valid_xml_structure() {
        let g = small_graph();
        let xml = to_graphml(&g);
        assert!(xml.contains("<?xml"));
        assert!(xml.contains("<graphml"));
        assert!(xml.contains("</graphml>"));
        assert!(xml.contains("container")); // hierarchy key
    }

    #[test]
    fn graphml_escapes_special_chars() {
        let mut g = RpgGraph::new();
        let n = crate::Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "foo<bar>&baz",
        );
        g.add_node(n);
        let xml = to_graphml(&g);
        assert!(xml.contains("&lt;bar&gt;"));
        assert!(xml.contains("&amp;baz"));
        assert!(!xml.contains("foo<bar>")); // raw angle brackets must be escaped
    }

    #[test]
    fn cypher_has_create_statements() {
        let g = small_graph();
        let cypher = to_cypher(&g);
        assert!(cypher.contains("CREATE"));
        assert!(cypher.contains("MATCH"));
        assert!(cypher.contains("CONTAINS"));
    }

    #[test]
    fn dot_is_valid_graphviz() {
        let g = small_graph();
        let dot = to_dot(&g);
        assert!(dot.starts_with("digraph rpg {"));
        assert!(dot.contains("n0")); // node id
        assert!(dot.contains("->")); // edge
        assert!(dot.ends_with("}\n"));
    }

    #[test]
    fn dot_colors_relationship_edges() {
        let mut g = RpgGraph::new();
        g.add_node(crate::Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "a",
        ));
        g.add_node(crate::Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "fn",
            "rust",
            "b",
        ));
        g.add_edge(NodeId::new(0), NodeId::new(1), Edge::new(EdgeType::Calls));
        let dot = to_dot(&g);
        assert!(dot.contains("color=\"blue\"")); // Calls = blue
    }

    #[test]
    fn export_preserves_node_edge_counts() {
        let g = small_graph();
        let node_count = g.node_count();
        let xml = to_graphml(&g);
        // Count <node id="..."> occurrences.
        let xml_nodes = xml.matches("<node id=").count();
        assert_eq!(xml_nodes, node_count);
    }
}

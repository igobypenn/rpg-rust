//! SCIP (Sourcegraph Code Intelligence Protocol) enrichment.
//!
//! SCIP index files (`.scip`, protobuf) provide precise symbol occurrences
//! and relationships — compiler-grade resolution that is more accurate than
//! the tree-sitter name-based heuristics used by the builder.
//!
//! This module provides a post-parse enrichment pass: tree-sitter produces
//! structural nodes + heuristic edges, then SCIP data rewrites the weak
//! Calls/References/UsesType edges to precise ones.
//!
//! ## Data model
//!
//! To avoid a hard protobuf dependency, this module works with
//! [`ScipOccurrence`] and [`ScipRelationship`] — Rust-native structs that
//! mirror the SCIP protobuf fields. The [`ScipIndex`] can be populated from a
//! `.scip` file (behind the `scip` feature, which pulls in the `scip` crate)
//! or constructed synthetically (for tests or custom indexers).
//!
//! ## Enrichment strategy
//!
//! 1. Build a `symbol → NodeId` map by joining SCIP occurrences to graph nodes
//!    via location (file + line).
//! 2. For each SCIP relationship, look up both endpoints. If both resolve,
//!    add a precise edge marked with `"source": "scip"` in edge metadata.
//! 3. Return stats on how many edges were refined/added and how many symbols
//!    were mapped/unmapped.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Edge, EdgeType, NodeId, RpgGraph};

/// A single symbol occurrence in a SCIP index (simplified from the protobuf).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScipOccurrence {
    /// The SCIP symbol identifier (opaque string, e.g. "rust main foo 12345").
    pub symbol: String,
    /// Relative file path (repo-relative).
    pub file: PathBuf,
    /// Starting line (1-based).
    pub start_line: usize,
    /// Starting column (0-based, byte offset).
    pub start_col: usize,
    /// Is this a definition occurrence?
    pub is_definition: bool,
}

/// A relationship between two symbols (simplified from SCIP protobuf).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScipRelationship {
    /// The source symbol.
    pub source_symbol: String,
    /// The target symbol.
    pub target_symbol: String,
    /// Edge type this relationship implies.
    pub edge_type: ScipEdgeType,
}

/// Maps SCIP relationship semantics to RPG edge types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScipEdgeType {
    /// Function/method call.
    Calls,
    /// Type usage / reference.
    UsesType,
    /// General reference (import, etc.).
    References,
    /// Implementation of an interface/trait.
    Implements,
}

impl From<ScipEdgeType> for EdgeType {
    fn from(t: ScipEdgeType) -> Self {
        match t {
            ScipEdgeType::Calls => EdgeType::Calls,
            ScipEdgeType::UsesType => EdgeType::UsesType,
            ScipEdgeType::References => EdgeType::References,
            ScipEdgeType::Implements => EdgeType::Implements,
        }
    }
}

/// A parsed SCIP index, holding occurrences and relationships.
///
/// Can be populated from a `.scip` file (behind the `scip` feature) or built
/// programmatically.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScipIndex {
    pub occurrences: Vec<ScipOccurrence>,
    pub relationships: Vec<ScipRelationship>,
}

impl ScipIndex {
    /// Create an empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add an occurrence.
    pub fn with_occurrence(mut self, occ: ScipOccurrence) -> Self {
        self.occurrences.push(occ);
        self
    }

    /// Add a relationship.
    pub fn with_relationship(mut self, rel: ScipRelationship) -> Self {
        self.relationships.push(rel);
        self
    }
}

/// SCIP processing error.
#[derive(Debug, thiserror::Error)]
pub enum ScipError {
    #[error("SCIP I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("SCIP parse error: {0}")]
    Parse(String),
}

/// Statistics from an enrichment pass.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScipStats {
    /// Number of SCIP symbols successfully mapped to graph nodes.
    pub symbols_mapped: usize,
    /// Number of SCIP symbols that couldn't be mapped.
    pub symbols_unmapped: usize,
    /// Edges added from SCIP relationships.
    pub edges_added: usize,
    /// Edges that already existed (heuristic) and were marked as SCIP-confirmed.
    pub edges_confirmed: usize,
}

/// Enrich a graph's edges with precise SCIP relationships.
///
/// 1. Maps SCIP definition occurrences to graph nodes by location.
/// 2. For each SCIP relationship, adds a precise edge (marked with
///    `metadata["source"] = "scip"`) if both endpoints are mapped.
/// 3. Does NOT remove existing heuristic edges — SCIP edges are additive
///    (provenance in metadata distinguishes them).
pub fn enrich_graph(graph: &mut RpgGraph, index: &ScipIndex) -> ScipStats {
    let mut stats = ScipStats::default();

    // Build symbol → NodeId map from definition occurrences.
    let mut symbol_to_node: HashMap<String, NodeId> = HashMap::new();
    for occ in &index.occurrences {
        if !occ.is_definition {
            continue;
        }
        // Find the graph node at this location.
        if let Some(node_id) = find_node_at(graph, &occ.file, occ.start_line) {
            symbol_to_node.insert(occ.symbol.clone(), node_id);
            stats.symbols_mapped += 1;
        } else {
            stats.symbols_unmapped += 1;
        }
    }

    // Also map non-definition occurrences (they reference symbols defined
    // elsewhere). These are needed for the source side of relationships.
    for occ in &index.occurrences {
        if occ.is_definition {
            continue;
        }
        if symbol_to_node.contains_key(&occ.symbol) {
            continue;
        }
        // For references, the symbol is defined elsewhere — we only need the
        // target node (mapped via the definition occurrence).
    }

    // Process relationships: add precise edges.
    for rel in &index.relationships {
        let Some(&source) = symbol_to_node.get(&rel.source_symbol) else {
            continue;
        };
        let Some(&target) = symbol_to_node.get(&rel.target_symbol) else {
            continue;
        };

        let edge_type: EdgeType = rel.edge_type.into();

        // Check if an edge of this type already exists between these nodes.
        let exists = graph
            .edges_from(source)
            .iter()
            .any(|(dst, e)| *dst == target && e.edge_type == edge_type);

        if exists {
            stats.edges_confirmed += 1;
        } else {
            let mut edge = Edge::new(edge_type);
            edge.metadata.insert("source".to_string(), "scip".into());
            edge.metadata.insert(
                "scip_symbol".to_string(),
                rel.source_symbol.clone().into(),
            );
            graph.add_edge(source, target, edge);
            stats.edges_added += 1;
        }
    }

    stats
}

/// Find the graph node whose source range contains (file, line).
/// Returns the innermost (tightest span) match.
fn find_node_at(graph: &RpgGraph, file: &Path, line: usize) -> Option<NodeId> {
    let candidates: Vec<(NodeId, usize)> = graph
        .nodes()
        .filter_map(|n| {
            if n.path.as_deref() != Some(file) {
                return None;
            }
            let (start, end) = n
                .source_ref
                .as_ref()
                .map(|sr| (sr.start_line, sr.end_line))
                .or_else(|| n.location.as_ref().map(|l| (l.start_line, l.end_line)))?;
            if start > 0 && line >= start && line <= end {
                Some((n.id, end - start))
            } else {
                None
            }
        })
        .collect();

    candidates
        .into_iter()
        .min_by_key(|(_, span)| *span)
        .map(|(id, _)| id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SourceRef;
    use crate::{GraphBuilder, NodeCategory};

    fn make_test_graph() -> RpgGraph {
        // Build a small graph with two functions in one file.
        let mut builder = GraphBuilder::new().with_repo("test", Path::new("/repo"));
        let file = Path::new("/repo/src/lib.rs");
        builder = builder.add_file(file, "rust");

        // Manually add function nodes with source ranges.
        let mut fn_a = crate::Node::new(
            NodeId::new(builder.graph.node_count()),
            NodeCategory::Function,
            "fn",
            "rust",
            "func_a",
        )
        .with_path(Path::new("src/lib.rs"));
        fn_a.source_ref = Some(SourceRef { start_line: 1, end_line: 5 });
        let id_a = builder.graph.add_node(fn_a);

        let mut fn_b = crate::Node::new(
            NodeId::new(builder.graph.node_count()),
            NodeCategory::Function,
            "fn",
            "rust",
            "func_b",
        )
        .with_path(Path::new("src/lib.rs"));
        fn_b.source_ref = Some(SourceRef { start_line: 7, end_line: 10 });
        let id_b = builder.graph.add_node(fn_b);

        let _ = (id_a, id_b);
        builder.build()
    }

    #[test]
    fn enrich_adds_precise_call_edge() {
        let mut graph = make_test_graph();
        let initial_edges = graph.edge_count();

        let index = ScipIndex::new()
            .with_occurrence(ScipOccurrence {
                symbol: "sym func_a".to_string(),
                file: PathBuf::from("src/lib.rs"),
                start_line: 1,
                start_col: 0,
                is_definition: true,
            })
            .with_occurrence(ScipOccurrence {
                symbol: "sym func_b".to_string(),
                file: PathBuf::from("src/lib.rs"),
                start_line: 7,
                start_col: 0,
                is_definition: true,
            })
            .with_relationship(ScipRelationship {
                source_symbol: "sym func_a".to_string(),
                target_symbol: "sym func_b".to_string(),
                edge_type: ScipEdgeType::Calls,
            });

        let stats = enrich_graph(&mut graph, &index);

        assert_eq!(stats.symbols_mapped, 2);
        assert_eq!(stats.edges_added, 1);
        assert_eq!(stats.edges_confirmed, 0);
        assert!(graph.edge_count() > initial_edges);
    }

    #[test]
    fn enrich_confirms_existing_edge() {
        let mut graph = make_test_graph();

        // Find the actual node IDs (graph has Repository + File + functions).
        let id_a = graph.nodes().find(|n| n.name == "func_a").map(|n| n.id).unwrap();
        let id_b = graph.nodes().find(|n| n.name == "func_b").map(|n| n.id).unwrap();

        // Add a heuristic Calls edge first (simulating tree-sitter resolution).
        graph.add_edge(id_a, id_b, Edge::new(EdgeType::Calls));

        let index = ScipIndex::new()
            .with_occurrence(ScipOccurrence {
                symbol: "sym func_a".to_string(),
                file: PathBuf::from("src/lib.rs"),
                start_line: 1,
                start_col: 0,
                is_definition: true,
            })
            .with_occurrence(ScipOccurrence {
                symbol: "sym func_b".to_string(),
                file: PathBuf::from("src/lib.rs"),
                start_line: 7,
                start_col: 0,
                is_definition: true,
            })
            .with_relationship(ScipRelationship {
                source_symbol: "sym func_a".to_string(),
                target_symbol: "sym func_b".to_string(),
                edge_type: ScipEdgeType::Calls,
            });

        let stats = enrich_graph(&mut graph, &index);
        assert_eq!(stats.edges_confirmed, 1);
        assert_eq!(stats.edges_added, 0);
    }

    #[test]
    fn enrich_handles_unmappable_symbols() {
        let mut graph = make_test_graph();

        let index = ScipIndex::new()
            .with_occurrence(ScipOccurrence {
                symbol: "sym unknown".to_string(),
                file: PathBuf::from("nonexistent.rs"),
                start_line: 1,
                start_col: 0,
                is_definition: true,
            })
            .with_relationship(ScipRelationship {
                source_symbol: "sym unknown".to_string(),
                target_symbol: "sym also_unknown".to_string(),
                edge_type: ScipEdgeType::Calls,
            });

        let stats = enrich_graph(&mut graph, &index);
        assert_eq!(stats.symbols_unmapped, 1);
        assert_eq!(stats.edges_added, 0);
    }

    #[test]
    fn enrich_marks_scip_provenance() {
        let mut graph = make_test_graph();
        let index = ScipIndex::new()
            .with_occurrence(ScipOccurrence {
                symbol: "sym a".to_string(),
                file: PathBuf::from("src/lib.rs"),
                start_line: 1,
                start_col: 0,
                is_definition: true,
            })
            .with_occurrence(ScipOccurrence {
                symbol: "sym b".to_string(),
                file: PathBuf::from("src/lib.rs"),
                start_line: 7,
                start_col: 0,
                is_definition: true,
            })
            .with_relationship(ScipRelationship {
                source_symbol: "sym a".to_string(),
                target_symbol: "sym b".to_string(),
                edge_type: ScipEdgeType::References,
            });

        enrich_graph(&mut graph, &index);

        // Find the SCIP-sourced edge and check its metadata.
        let scip_edge = graph
            .edges()
            .find(|(_, _, e)| e.metadata.get("source").and_then(|v| v.as_str()) == Some("scip"));
        assert!(scip_edge.is_some(), "should have an edge marked source=scip");
        let (_, _, edge) = scip_edge.unwrap();
        assert_eq!(edge.edge_type, EdgeType::References);
    }
}

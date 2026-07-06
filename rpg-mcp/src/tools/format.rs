//! Shared response formatting for MCP tools.
//!
//! Provides the `detail_level` ladder (minimal → summary → full) that controls
//! how much data each tool returns, plus node/edge → JSON shaping and a
//! source-line reader. Centralizing this keeps tool responses consistent and
//! lets agents control token budgets.

use std::path::Path;

use rpg_encoder::{Edge, EdgeType, Node, NodeCategory, NodeId, NodeLevel};
use serde_json::{json, Value};

/// Verbosity of node data in tool responses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DetailLevel {
    /// id, name, kind, category, path only. Smallest payload.
    #[default]
    Minimal,
    /// minimal + signature, description, features, language.
    Summary,
    /// summary + metadata, location, source_ref, node_level, documentation,
    /// semantic_feature, feature_path. Everything.
    Full,
}

impl DetailLevel {
    /// Parse from a string param (case-insensitive). Unknown → Minimal.
    pub fn parse(s: Option<&str>) -> Self {
        match s.map(str::to_ascii_lowercase).as_deref() {
            Some("summary") => Self::Summary,
            Some("full") => Self::Full,
            _ => Self::Minimal,
        }
    }
}

/// Shape a node into a JSON value at the given detail level.
#[must_use]
pub fn node_to_json(node: &Node, level: DetailLevel) -> Value {
    let base = json!({
        "id": node.id.index(),
        "name": node.name,
        "kind": node.kind,
        "category": node.category.to_string(),
        "path": node.path,
    });

    match level {
        DetailLevel::Minimal => base,
        DetailLevel::Summary => {
            let mut v = base;
            v["language"] = json!(node.language);
            if let Some(ref sig) = node.signature {
                v["signature"] = json!(sig);
            }
            if let Some(ref desc) = node.description {
                if !desc.is_empty() {
                    v["description"] = json!(desc);
                }
            }
            if !node.features.is_empty() {
                v["features"] = json!(node.features);
            }
            v
        }
        DetailLevel::Full => {
            let mut v = node_to_json(node, DetailLevel::Summary);
            v["node_level"] = json!(node.node_level.to_string());
            if let Some(ref sf) = node.semantic_feature {
                v["semantic_feature"] = json!(sf);
            }
            if let Some(ref fp) = node.feature_path {
                v["feature_path"] = json!(fp);
            }
            if let Some(ref doc) = node.documentation {
                v["documentation"] = json!(doc);
            }
            // location (file + line/col) and source_ref (line range)
            if let Some(ref loc) = node.location {
                v["location"] = json!({
                    "file": loc.file,
                    "start_line": loc.start_line,
                    "start_column": loc.start_column,
                    "end_line": loc.end_line,
                    "end_column": loc.end_column,
                });
            }
            if let Some(ref sr) = node.source_ref {
                v["source_ref"] = json!({
                    "start_line": sr.start_line,
                    "end_line": sr.end_line,
                });
            }
            if !node.metadata.is_empty() {
                v["metadata"] = json!(node.metadata);
            }
            v
        }
    }
}

/// Shape an edge into a JSON value.
#[must_use]
pub fn edge_to_json(source: NodeId, target: NodeId, edge: &Edge) -> Value {
    let mut v = json!({
        "source": source.index(),
        "target": target.index(),
        "type": edge.edge_type.to_string(),
    });
    if !edge.metadata.is_empty() {
        v["metadata"] = json!(edge.metadata);
    }
    v
}

/// Read a slice of source lines for a node.
///
/// Uses `repo_dir` to resolve the node's (repo-relative) path, then slices
/// the file's lines by the node's location or source_ref. `context` adds
/// extra lines before/after the node's range.
///
/// Returns `None` if the node has no path, no line info, or the file can't be
/// read.
pub fn read_node_source(
    node: &Node,
    repo_dir: &Path,
    context_lines: usize,
) -> Option<(String, usize, usize)> {
    let path = node.path.as_ref()?;
    let full = repo_dir.join(path);

    // Prefer source_ref (line range), fall back to location.
    let (start, end) = node
        .source_ref
        .as_ref()
        .map(|sr| (sr.start_line, sr.end_line))
        .or_else(|| {
            node.location
                .as_ref()
                .map(|l| (l.start_line, l.end_line))
        })?;

    if start == 0 || end == 0 {
        return None;
    }

    let content = std::fs::read_to_string(&full).ok()?;
    let lines: Vec<&str> = content.lines().collect();

    let lo = start.saturating_sub(1).saturating_sub(context_lines);
    let hi = (end + context_lines).min(lines.len());
    if lo >= hi || hi > lines.len() {
        return None;
    }

    let slice: Vec<&str> = lines[lo..hi].to_vec();
    Some((slice.join("\n"), lo + 1, hi))
}

/// Count incoming edges of a specific type for a node (used by dead-code /
/// hub analysis). Cheap helper over a node's edge list.
pub fn count_incoming_by_type(
    edges_to: &[(NodeId, &Edge)],
    edge_type: EdgeType,
) -> usize {
    edges_to
        .iter()
        .filter(|(_, e)| e.edge_type == edge_type)
        .count()
}

/// Convenience: is this node a "leaf" definition (Function/Type/Method)?
/// Used to scope dead-code and impact analysis to meaningful targets.
pub fn is_definition(category: NodeCategory) -> bool {
    matches!(
        category,
        NodeCategory::Function | NodeCategory::Type | NodeCategory::Field
    )
}

/// Format a node_level for JSON output.
#[must_use]
pub fn level_str(level: NodeLevel) -> &'static str {
    match level {
        NodeLevel::Low => "low",
        NodeLevel::Intermediate => "intermediate",
        NodeLevel::High => "high",
    }
}

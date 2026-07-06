//! Per-node embed-text construction.
//!
//! Builds the string sent to the embedding model for each node. Low-level nodes
//! embed metadata + their own source lines; centroids (V^H) embed metadata +
//! their grounded `semantic_feature` (no source — they're virtual).
//!
//! Source reading mirrors `rpg_mcp::tools::format::read_node_source` but lives
//! here because rpg-encoder cannot depend on rpg-mcp (the dependency direction
//! is reversed). The two are kept deliberately similar so behavior matches.

use std::path::Path;

use crate::{Node, NodeLevel};

/// Truncation cap for embedded text. Qwen3-Embedding-8B accepts 8K tokens;
/// ~6000 chars leaves headroom for the model's tokenizer and avoids oversized
/// requests on large functions.
const MAX_TEXT_CHARS: usize = 6000;

/// Build the embed text for a node.
///
/// - Low-level node: `"{name} ({kind}, {language})\n{signature}\nfeatures: ...\ndesc: ...\n--- source ---\n{source}"`
/// - Centroid (V^H): metadata + `semantic_feature` only.
///
/// Returns an empty string if the node has no semantic content.
#[must_use]
pub fn node_embed_text(node: &Node, repo_dir: &Path) -> String {
    let mut s = String::new();
    s.push_str(&format!("{} ({}, {})\n", node.name, node.kind, node.language));

    if let Some(ref sig) = node.signature {
        s.push_str(sig);
        s.push('\n');
    }

    if !node.features.is_empty() {
        s.push_str("features: ");
        s.push_str(&node.features.join(", "));
        s.push('\n');
    }

    if let Some(ref sf) = node.semantic_feature {
        s.push_str("semantic: ");
        s.push_str(sf);
        s.push('\n');
    }

    if let Some(ref desc) = node.description {
        if !desc.is_empty() {
            s.push_str("desc: ");
            s.push_str(desc);
            s.push('\n');
        }
    }

    // Centroids and other V^H nodes are virtual — no source lines to embed.
    if node.node_level != NodeLevel::High {
        if let Some(src) = read_node_source(node, repo_dir) {
            s.push_str("--- source ---\n");
            s.push_str(&src);
        }
    }

    if s.len() > MAX_TEXT_CHARS {
        // Step back to the nearest char boundary — String::truncate panics if
        // the cut point splits a multi-byte UTF-8 sequence (common with CJK
        // identifiers or emoji in source comments).
        let mut cut = MAX_TEXT_CHARS;
        while cut > 0 && !s.is_char_boundary(cut) {
            cut -= 1;
        }
        s.truncate(cut);
    }
    s
}

/// Read the source lines a node spans. Returns `None` if the node has no path,
/// no line info, or the file can't be read.
///
/// Mirrors `read_node_source` in rpg-mcp/tools/format.rs but without the
/// context-lines parameter (embeddings always use the exact node span).
fn read_node_source(node: &Node, repo_dir: &Path) -> Option<String> {
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

    let lo = start.saturating_sub(1);
    let hi = end.min(lines.len());
    if lo >= hi || hi > lines.len() {
        return None;
    }

    Some(lines[lo..hi].join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::SourceRef;
    use crate::{Node, NodeCategory, NodeId, NodeLevel};
    use std::io::Write;

    #[test]
    fn embed_text_includes_source() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("lib.rs");
        let mut f = std::fs::File::create(&file).unwrap();
        f.write_all(b"line0\nfn foo() {}\nline2\n").unwrap();

        let mut node = Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "fn",
            "rust",
            "foo",
        )
        .with_signature("fn foo()".to_string())
        .with_features(vec!["does foo".to_string()])
        .with_description("A foo function".to_string());
        node.path = Some(file.strip_prefix(dir.path()).unwrap().to_path_buf());
        node.source_ref = Some(SourceRef {
            start_line: 2,
            end_line: 2,
        });

        let text = node_embed_text(&node, dir.path());
        assert!(text.contains("foo (fn, rust)"));
        assert!(text.contains("fn foo()"));
        assert!(text.contains("does foo"));
        assert!(text.contains("A foo function"));
        assert!(text.contains("--- source ---"));
        assert!(text.contains("fn foo() {}"));
    }

    #[test]
    fn centroid_has_no_source() {
        let mut node = Node::new(
            NodeId::new(0),
            NodeCategory::FunctionalCentroid,
            "centroid",
            "rust",
            "Auth",
        )
        .with_semantic_feature("Handles login and sessions".to_string());
        node.node_level = NodeLevel::High;
        // Even with a path/source_ref set, a High node omits source.
        node.path = Some(std::path::PathBuf::from("x.rs"));
        node.source_ref = Some(SourceRef {
            start_line: 1,
            end_line: 1,
        });

        let text = node_embed_text(&node, Path::new("/repo"));
        assert!(text.contains("Handles login and sessions"));
        assert!(!text.contains("--- source ---"));
    }

    #[test]
    fn truncates_long_text() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.rs");
        let mut f = std::fs::File::create(&file).unwrap();
        // 10000 chars of source.
        let big = "x".repeat(10_000);
        write!(f, "{big}").unwrap();

        let mut node = Node::new(
            NodeId::new(0),
            NodeCategory::Type,
            "struct",
            "rust",
            "Big",
        );
        node.path = Some(file.strip_prefix(dir.path()).unwrap().to_path_buf());
        node.source_ref = Some(SourceRef {
            start_line: 1,
            end_line: 1,
        });

        let text = node_embed_text(&node, dir.path());
        assert!(text.len() <= MAX_TEXT_CHARS + 64); // +64 for metadata head
    }
}

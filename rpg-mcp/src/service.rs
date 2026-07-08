use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rmcp::{
    handler::server::router::tool::ToolRouter, model::*, tool, tool_handler, tool_router,
    ErrorData as McpError, ServerHandler,
};
use rpg_encoder::encoder::ValidationReport;
use rpg_encoder::{
    EdgeType, Embedder, EmbeddingIndex, NodeCategory, NodeId, RpgEncoder, RpgSnapshot, RpgStore,
};
use serde_json::{json, Map, Value};

use crate::state::{compute_dir_hash, load_dir_hash, save_dir_hash, AppState};
use crate::tools::format::node_to_json;

type JsonObject = Map<String, Value>;

fn get_str<'a>(params: &'a JsonObject, key: &str) -> Option<&'a str> {
    params.get(key).and_then(|v| v.as_str())
}

fn get_u64(params: &JsonObject, key: &str) -> Option<u64> {
    params.get(key).and_then(|v| v.as_u64())
}

/// Parse the detail_level ladder: minimal → summary → full.
fn parse_detail_level(s: &str) -> crate::tools::format::DetailLevel {
    crate::tools::format::DetailLevel::parse(Some(s))
}

/// Maximum number of results any tool returns in a single response.
/// Prevents MCP protocol bloat on large graphs (CRG issue #262).
const MAX_RESULTS: usize = 500;

/// Resolve a node id param to a NodeId, returning invalid_params if the node
/// doesn't exist. Eliminates the duplicated not-found check across tools.
fn require_node(graph: &rpg_encoder::RpgGraph, id: u64) -> Result<rpg_encoder::NodeId, McpError> {
    let nid = rpg_encoder::NodeId::new(id as usize);
    graph
        .get_node(nid)
        .ok_or_else(|| McpError::invalid_params(format!("node {id} not found"), None))
        .map(|_| nid)
}

#[derive(Clone)]
pub struct RpgService {
    state: Arc<AppState>,
    tool_router: ToolRouter<RpgService>,
    telemetry: Arc<crate::tools::telemetry::Telemetry>,
}

#[tool_router]
impl RpgService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
            telemetry: Arc::new(crate::tools::telemetry::Telemetry::from_env()),
        }
    }

    /// Construct with explicit telemetry config (for tests).
    pub fn with_telemetry(state: Arc<AppState>, telemetry: crate::tools::telemetry::Telemetry) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
            telemetry: Arc::new(telemetry),
        }
    }

    #[tool(description = "Full re-encode of the workspace repository")]
    pub async fn encode_repo(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let _tel = crate::tools::telemetry::timed(&self.telemetry, "encode_repo", &params);
        let workspace = self.state.config.workspace.clone();

        let mut encoder = RpgEncoder::new()
            .map_err(|e| McpError::internal_error(format!("Encoder init failed: {e}"), None))?;

        let result = encoder
            .encode(&workspace)
            .map_err(|e| McpError::internal_error(format!("Encode failed: {e}"), None))?;

        let mut snapshot = RpgSnapshot::from_encoder(&encoder);
        snapshot.compute_file_hashes().map_err(|e| {
            McpError::internal_error(format!("Compute file hashes failed: {e}"), None)
        })?;
        snapshot.build_reverse_deps();

        let mut store_guard = self.state.store.write();
        if store_guard.is_none() {
            match RpgStore::open(&workspace) {
                Ok(s) => *store_guard = Some(s),
                Err(_) => {
                    let s = RpgStore::init(&workspace).map_err(|e| {
                        McpError::internal_error(format!("Store init failed: {e}"), None)
                    })?;
                    *store_guard = Some(s);
                }
            }
        }

        if let Some(store) = store_guard.as_mut() {
            store
                .save_base(&snapshot)
                .map_err(|e| McpError::internal_error(format!("Save base failed: {e}"), None))?;
        }
        drop(store_guard);

        if let Err(e) = save_dir_hash(
            &self.state.config.data_dir,
            &compute_dir_hash(&workspace, self.state.config.hash_mode)
                .map_err(|e| McpError::internal_error(format!("Dir hash failed: {e}"), None))?,
        ) {
            tracing::warn!("Failed to save dir hash: {}", e);
        }

        let total_nodes = snapshot.graph.node_count();
        let total_edges = snapshot.graph.edge_count();
        self.state.update(snapshot);

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "status": "ok",
                "files_processed": result.files_processed,
                "files_skipped": result.files_skipped,
                "parse_errors": result.parse_errors.len(),
                "total_nodes": total_nodes,
                "total_edges": total_edges,
            })
            .to_string(),
        )]))
    }

    /// Compute vector embeddings over every embeddable node in the current
    /// graph and persist them to `<workspace>/.rpg/embeddings.bin`.
    ///
    /// Requires the graph to carry LLM semantic features (run a semantic encode
    /// first). The hash cache is persisted to embeddings_hashes.bin, so
    /// re-running skips nodes whose embed text hasn't changed.
    #[tool(
        description = "Compute vector embeddings over graph nodes (metadata + source). Stores to .rpg/embeddings.bin. Run after a semantic encode. Required before vector_search works."
    )]
    pub async fn encode_embeddings(&self, _params: JsonObject) -> Result<CallToolResult, McpError> {
        use rpg_encoder::{EmbeddingClient, EmbeddingConfig, EmbeddingStore, FlatIndex};

        let workspace = self.state.config.workspace.clone();
        let emb_config = EmbeddingConfig::from_env();

        // Build the store: re-open an existing sidecar (incremental) or create.
        let sidecar = self.state.config.data_dir.join("embeddings.bin");
        let mut store = if sidecar.exists() {
            let idx = FlatIndex::load(&sidecar).map_err(|e| {
                McpError::internal_error(format!("Load embeddings sidecar: {e}"), None)
            })?;
            EmbeddingStore::new(Box::new(idx), sidecar.clone())
        } else {
            EmbeddingStore::new(
                Box::new(FlatIndex::new(emb_config.dimension)),
                sidecar.clone(),
            )
        };

        let client = EmbeddingClient::new(emb_config.clone())
            .map_err(|e| McpError::internal_error(format!("Embedding client init: {e}"), None))?;

        // Snapshot the graph by clone: embed_graph is async and the lock guard
        // isn't Send, so we can't hold it across the embedding awaits. The
        // clone is cheap relative to the embedding HTTP cost.
        let graph = self.state.graph.read().clone();

        // embed_graph hits the network: a down/unreachable endpoint is an
        // expected runtime condition, so surface it as a JSON error body
        // rather than an MCP internal-error (which signals a server bug).
        let stats = match store
            .embed_graph(&graph, &workspace, &client, emb_config.batch_size)
            .await
        {
            Ok(s) => s,
            Err(e) => {
                return Ok(CallToolResult::success(vec![Content::text(
                    json!({
                        "status": "error",
                        "error": format!("embedding request failed: {e}"),
                        "endpoint": emb_config.endpoint,
                    })
                    .to_string(),
                )]));
            }
        };

        // Cache the freshly-built index for subsequent vector_search calls.
        let count = store.len();
        // Persist is already done inside embed_graph, but save once more to be
        // safe when nothing changed (no-op when path is set).
        if let Err(e) = store.save() {
            tracing::warn!("Failed to save embeddings sidecar: {e}");
        }
        drop(store);

        // Load the index into the AppState cache so vector_search skips the
        // load-on-first-use path.
        if let Ok(idx) = FlatIndex::load(&sidecar) {
            *self.state.embeddings.write() = Some(idx);
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "status": "ok",
                "embedded": stats.embedded,
                "skipped_unchanged": stats.skipped_unchanged,
                "queued": stats.queued,
                "total_vectors": count,
                "backend": "flat",
                "sidecar": sidecar.display().to_string(),
            })
            .to_string(),
        )]))
    }

    /// Detect file changes since the last encode and incrementally re-encode.
    ///
    /// Compares the current directory hash against the stored hash. If they
    /// differ, runs `generate_diff` + `RpgEvolution::process_diff` to patch
    /// only the added/deleted/modified files into the existing graph. Returns
    /// the change summary. If nothing changed, returns `"changed": false`.
    ///
    /// This is the on-demand equivalent of the file watcher — callers can poll
    /// it after edits to refresh the graph without a full re-encode.
    #[tool(
        description = "Detect file changes and incrementally re-encode (only added/deleted/modified files). On-demand refresh — cheaper than encode_repo when few files changed. Returns change summary."
    )]
    pub async fn detect_changes(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let _tel = crate::tools::telemetry::timed(&self.telemetry, "detect_changes", &params);
        use rpg_encoder::{generate_diff, RpgEvolution};

        let workspace = self.state.config.workspace.clone();

        // Fast path: compare dir hashes. If unchanged, skip the diff entirely.
        let current_hash = compute_dir_hash(&workspace, self.state.config.hash_mode)
            .map_err(|e| McpError::internal_error(format!("Dir hash failed: {e}"), None))?;
        let stored_hash = load_dir_hash(&self.state.config.data_dir);
        if stored_hash.as_deref() == Some(current_hash.as_str()) {
            return Ok(CallToolResult::success(vec![Content::text(
                json!({
                    "changed": false,
                    "hash": current_hash,
                })
                .to_string(),
            )]));
        }

        // Slow path: something changed — run the incremental diff + evolution.
        // process_diff is async and borrows the snapshot mutably, so we hold
        // the write lock for the duration and drive the future via
        // block_in_place (same pattern as the file watcher).
        let (summary, graph_node_count, graph_edge_count) = {
            let mut snapshot = self.state.snapshot.write();
            let diff = generate_diff(&snapshot, &workspace, &self.state.registry)
                .map_err(|e| McpError::internal_error(format!("Diff failed: {e}"), None))?;

            if diff.is_empty() {
                // Hash differed but no code units changed (e.g. whitespace in a
                // non-code file). Persist the new hash so we don't re-scan
                // on every subsequent call.
                drop(snapshot);
                if let Err(e) = save_dir_hash(&self.state.config.data_dir, &current_hash) {
                    tracing::warn!("Failed to save dir hash: {}", e);
                }
                return Ok(CallToolResult::success(vec![Content::text(
                    json!({
                        "changed": false,
                        "hash": current_hash,
                        "note": "hash mismatch but no code units changed",
                    })
                    .to_string(),
                )]));
            }

            let files_added = diff.added.len();
            let files_deleted = diff.deleted.len();
            let files_modified = diff.modified.len();

            let mut evolution = RpgEvolution::new(&mut snapshot, &self.state.registry);
            let summary = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current()
                    .block_on(evolution.process_diff(diff, None))
            })
            .map_err(|e| McpError::internal_error(format!("Evolution failed: {e}"), None))?;

            let new_graph = snapshot.graph.clone();
            let (nc, ec) = (new_graph.node_count(), new_graph.edge_count());
            // CRITICAL: drop the snapshot write lock BEFORE acquiring the
            // graph write lock. Holding both creates an AB-BA deadlock with
            // read tools that acquire graph.read() then snapshot.read().
            // The watcher uses AppState::update() which does the same
            // (snapshot.write → clone → drop → graph.write).
            drop(snapshot);
            *self.state.graph.write() = new_graph;

            (
                json!({
                    "files_added": files_added,
                    "files_deleted": files_deleted,
                    "files_modified": files_modified,
                    "units_added": summary.units_added,
                    "units_deleted": summary.units_deleted,
                    "units_changed": summary.units_changed,
                    "nodes_created": summary.nodes_created,
                    "nodes_removed": summary.nodes_removed,
                    "nodes_updated": summary.nodes_updated,
                }),
                nc,
                ec,
            )
        };

        // Persist the new snapshot + update the stored hash.
        {
            let snapshot = self.state.snapshot.read();
            let mut store_guard = self.state.store.write();
            if let Some(store) = store_guard.as_mut() {
                let _ = store.save_base(&snapshot);
            }
        }
        if let Err(e) = save_dir_hash(&self.state.config.data_dir, &current_hash) {
            tracing::warn!("Failed to save dir hash: {}", e);
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "changed": true,
                "summary": summary,
                "total_nodes": graph_node_count,
                "total_edges": graph_edge_count,
            })
            .to_string(),
        )]))
    }

    #[tool(description = "Get graph summary with validation report and language list")]
    pub async fn get_graph_summary(&self, _params: JsonObject) -> Result<CallToolResult, McpError> {
        let graph = self.state.graph.read();
        let report = ValidationReport::from_graph(&graph);

        let mut languages: HashSet<String> = HashSet::new();
        for node in graph.nodes() {
            if !node.language.is_empty() {
                languages.insert(node.language.clone());
            }
        }
        let mut languages: Vec<String> = languages.into_iter().collect();
        languages.sort();

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "total_nodes": report.total_nodes,
                "total_edges": report.total_edges,
                "edge_type_counts": report.edge_type_counts,
                "node_category_counts": report.node_category_counts,
                "import_resolution_rate": report.import_resolution_rate,
                "call_edge_count": report.call_edge_count,
                "implements_edge_count": report.implements_edge_count,
                "ffi_edge_count": report.ffi_edge_count,
                "warnings": report.warnings,
                "languages": languages,
            })
            .to_string(),
        )]))
    }

    #[tool(description = "Search nodes by name substring, with optional kind/category filters")]
    pub async fn search_nodes(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let _tel = crate::tools::telemetry::timed(&self.telemetry, "search_nodes", &params);
        let query = get_str(&params, "query")
            .ok_or_else(|| McpError::invalid_params("missing 'query'", None))?;
        let kind = get_str(&params, "kind");
        let category = get_str(&params, "category");
        let limit = get_u64(&params, "limit").unwrap_or(50).min(500) as usize;

        let cat_filter: Option<NodeCategory> = category.and_then(|c| {
            let parsed = parse_category(c);
            if parsed.is_none() {
                // Don't silently widen the search — let the caller know.
                tracing::warn!(category = c, "unknown category filter, ignoring");
            }
            parsed
        });
        let query_lower = query.to_lowercase();

        // Reject empty query — `"".contains("")` matches every node.
        if query_lower.is_empty() {
            return Err(McpError::invalid_params(
                "query must not be empty",
                None,
            ));
        }

        let graph = self.state.graph.read();
        let mut results: Vec<Value> = Vec::new();

        for node in graph.nodes() {
            if !node.name.to_lowercase().contains(&query_lower) {
                continue;
            }
            if let Some(k) = kind {
                if node.kind != k {
                    continue;
                }
            }
            if let Some(ref c) = cat_filter {
                if node.category != *c {
                    continue;
                }
            }

            results.push(json!({
                "id": node.id.index(),
                "name": node.name,
                "kind": node.kind,
                "category": node.category.to_string(),
                "language": node.language,
                "path": node.path,
            }));

            if results.len() >= limit {
                break;
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({ "nodes": results, "count": results.len() }).to_string(),
        )]))
    }

    #[tool(
        description = "Get full details for a node by index, including incoming and outgoing edges. Supports detail_level (minimal/summary/full) and include_source to read the node's source lines."
    )]
    pub async fn get_node_details(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        use crate::tools::format::{node_to_json, read_node_source, DetailLevel};

        let id = get_u64(&params, "id")
            .ok_or_else(|| McpError::invalid_params("missing numeric 'id'", None))?;
        let detail = DetailLevel::parse(get_str(&params, "detail_level"));
        let include_source = get_str(&params, "include_source")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);
        let context_lines = get_u64(&params, "context_lines").unwrap_or(0) as usize;

        let graph = self.state.graph.read();
        let node_id = require_node(&graph, id)?;
        let node = graph.get_node(node_id).unwrap();

        // Use the O(degree) edges_from/edges_to primitives instead of scanning
        // all edges. Also surface edge metadata (was dropped before).
        let incoming: Vec<Value> = graph
            .edges_to(node_id)
            .into_iter()
            .map(|(src, edge)| json!({
                "type": edge.edge_type.to_string(),
                "source": src.index(),
                "metadata": edge.metadata,
            }))
            .collect();
        let outgoing: Vec<Value> = graph
            .edges_from(node_id)
            .into_iter()
            .map(|(tgt, edge)| json!({
                "type": edge.edge_type.to_string(),
                "target": tgt.index(),
                "metadata": edge.metadata,
            }))
            .collect();

        let mut result = node_to_json(node, detail);
        result["incoming_edges"] = json!(incoming);
        result["outgoing_edges"] = json!(outgoing);

        if include_source {
            let snapshot = self.state.snapshot.read();
            if let Some((content, start, end)) = read_node_source(node, &snapshot.repo_dir, context_lines) {
                result["source"] = json!({
                    "content": content,
                    "start_line": start,
                    "end_line": end,
                });
            }
        }

        Ok(CallToolResult::success(vec![Content::text(result.to_string())]))
    }

    #[tool(description = "Get edges filtered by source/target/type, with limit")]
    pub async fn get_edges(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let source = get_u64(&params, "source").map(|v| v as usize);
        let target = get_u64(&params, "target").map(|v| v as usize);
        let edge_type = get_str(&params, "edge_type");
        let limit = get_u64(&params, "limit").unwrap_or(100).min(500) as usize;

        let type_filter: Option<EdgeType> = edge_type.and_then(parse_edge_type);

        let graph = self.state.graph.read();
        let mut results: Vec<Value> = Vec::new();

        for (src, tgt, edge) in graph.edges() {
            if let Some(s) = source {
                if src.index() != s {
                    continue;
                }
            }
            if let Some(t) = target {
                if tgt.index() != t {
                    continue;
                }
            }
            if let Some(ref et) = type_filter {
                if edge.edge_type != *et {
                    continue;
                }
            }

            results.push(json!({
                "source": src.index(),
                "target": tgt.index(),
                "type": edge.edge_type.to_string(),
            }));

            if results.len() >= limit {
                break;
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({ "edges": results, "count": results.len() }).to_string(),
        )]))
    }

    #[tool(
        description = "Traverse the graph from a starting node (BFS). Returns reachable nodes and edges up to a depth limit. direction: upstream (callers/dependents), downstream (callees/dependencies), or both. Optional edge_types filter."
    )]
    pub async fn explore_graph(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let start = get_u64(&params, "start_node")
            .ok_or_else(|| McpError::invalid_params("missing numeric 'start_node'", None))?
            as usize;
        let direction = get_str(&params, "direction").unwrap_or("both");
        let depth = get_u64(&params, "depth").unwrap_or(2).clamp(1, 10) as usize;
        let limit = get_u64(&params, "limit").unwrap_or(100).min(500) as usize;
        let edge_type_str = get_str(&params, "edge_type");
        let edge_filter = edge_type_str.and_then(parse_edge_type);

        let graph = self.state.graph.read();
        let start_id = rpg_encoder::NodeId::new(start);

        if graph.get_node(start_id).is_none() {
            return Err(McpError::invalid_params(
                format!("node {start} not found"),
                None,
            ));
        }

        let do_downstream = matches!(direction, "downstream" | "both");
        let do_upstream = matches!(direction, "upstream" | "both");

        // BFS frontier: (node_id, depth).
        let mut visited: std::collections::HashSet<rpg_encoder::NodeId> =
            std::collections::HashSet::new();
        visited.insert(start_id);
        let mut frontier: Vec<(rpg_encoder::NodeId, usize)> = vec![(start_id, 0)];
        let mut nodes_out: Vec<Value> = Vec::new();
        let mut edges_out: Vec<Value> = Vec::new();
        let mut depth_reached = 0usize;

        while let Some((node_id, d)) = frontier.pop() {
            if d >= depth {
                continue;
            }
            // Bound traversal by visited count (not nodes_out which is
            // populated only after the loop).
            if visited.len() >= limit {
                break;
            }

            // Gather outgoing (downstream) and/or incoming (upstream) edges.
            let mut expand = |tgt: rpg_encoder::NodeId, et: rpg_encoder::EdgeType, is_out: bool| {
                if let Some(f) = edge_filter {
                    if et != f {
                        return;
                    }
                }
                let (s, t) = if is_out { (node_id, tgt) } else { (tgt, node_id) };
                if edges_out.len() < limit {
                    edges_out.push(json!({
                        "source": s.index(),
                        "target": t.index(),
                        "type": et.to_string(),
                    }));
                }
                if !visited.contains(&tgt) && visited.len() < limit {
                    visited.insert(tgt);
                    frontier.push((tgt, d + 1));
                    depth_reached = depth_reached.max(d + 1);
                }
            };

            if do_downstream {
                for (tgt, edge) in graph.edges_from(node_id) {
                    expand(tgt, edge.edge_type, true);
                }
            }
            if do_upstream {
                for (src, edge) in graph.edges_to(node_id) {
                    expand(src, edge.edge_type, false);
                }
            }
        }

        // Shape visited nodes.
        for &id in &visited {
            if let Some(n) = graph.get_node(id) {
                nodes_out.push(json!({
                    "id": n.id.index(),
                    "name": n.name,
                    "kind": n.kind,
                    "category": n.category.to_string(),
                    "path": n.path,
                }));
                if nodes_out.len() >= limit {
                    break;
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "nodes": nodes_out,
                "edges": edges_out,
                "node_count": nodes_out.len(),
                "edge_count": edges_out.len(),
                "depth_reached": depth_reached,
            })
            .to_string(),
        )]))
    }

    #[tool(
        description = "Get all callers of a function/method (incoming Calls edges). depth>1 expands transitively."
    )]
    pub async fn get_callers(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let id = get_u64(&params, "node_id")
            .ok_or_else(|| McpError::invalid_params("missing numeric 'node_id'", None))?
            as usize;
        let depth = get_u64(&params, "depth").unwrap_or(1).clamp(1, 5) as usize;
        let limit = get_u64(&params, "limit").unwrap_or(50).min(500) as usize;

        let graph = self.state.graph.read();
        let node_id = rpg_encoder::NodeId::new(id);
        if graph.get_node(node_id).is_none() {
            return Err(McpError::invalid_params(format!("node {id} not found"), None));
        }

        let callers = transitive_direction(&graph, node_id, depth, limit, true);
        Ok(CallToolResult::success(vec![Content::text(
            json!({ "callers": callers, "count": callers.len() }).to_string(),
        )]))
    }

    #[tool(
        description = "Get all callees of a function/method (outgoing Calls edges). depth>1 expands transitively."
    )]
    pub async fn get_callees(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let id = get_u64(&params, "node_id")
            .ok_or_else(|| McpError::invalid_params("missing numeric 'node_id'", None))?
            as usize;
        let depth = get_u64(&params, "depth").unwrap_or(1).clamp(1, 5) as usize;
        let limit = get_u64(&params, "limit").unwrap_or(50).min(500) as usize;

        let graph = self.state.graph.read();
        let node_id = rpg_encoder::NodeId::new(id);
        if graph.get_node(node_id).is_none() {
            return Err(McpError::invalid_params(format!("node {id} not found"), None));
        }

        let callees = transitive_direction(&graph, node_id, depth, limit, false);
        Ok(CallToolResult::success(vec![Content::text(
            json!({ "callees": callees, "count": callees.len() }).to_string(),
        )]))
    }

    #[tool(
        description = "Read the source code lines for a node. Returns the node's line range plus optional context_lines before/after."
    )]
    pub async fn get_source(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        use crate::tools::format::read_node_source;

        let id = get_u64(&params, "node_id")
            .ok_or_else(|| McpError::invalid_params("missing numeric 'node_id'", None))?;
        let context_lines = get_u64(&params, "context_lines").unwrap_or(0) as usize;

        let graph = self.state.graph.read();
        let snapshot = self.state.snapshot.read();
        let node_id = require_node(&graph, id)?;
        let node = graph.get_node(node_id).unwrap();

        match read_node_source(node, &snapshot.repo_dir, context_lines) {
            Some((content, start, end)) => Ok(CallToolResult::success(vec![Content::text(
                json!({
                    "node_id": id,
                    "name": node.name,
                    "file": node.path,
                    "start_line": start,
                    "end_line": end,
                    "content": content,
                })
                .to_string(),
            )])),
            None => Ok(CallToolResult::success(vec![Content::text(
                json!({
                    "node_id": id,
                    "name": node.name,
                    "error": "no source available (node has no path, no line info, or file unreadable)",
                })
                .to_string(),
            )])),
        }
    }

    #[tool(
        description = "Compute the blast radius of changing one or more nodes. Returns callers (upstream dependents), callees (downstream dependencies), and the set of affected files. depth controls transitive expansion (default 2)."
    )]
    pub async fn get_impact(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let node_ids_json = params
            .get("node_ids")
            .and_then(|v| v.as_array())
            .ok_or_else(|| McpError::invalid_params("missing 'node_ids' array", None))?;
        let depth = get_u64(&params, "depth").unwrap_or(2).clamp(1, 5) as usize;

        let start_ids: Vec<rpg_encoder::NodeId> = node_ids_json
            .iter()
            .filter_map(|v| v.as_u64().map(|i| rpg_encoder::NodeId::new(i as usize)))
            .collect();
        if start_ids.is_empty() {
            return Err(McpError::invalid_params("'node_ids' must be non-empty", None));
        }

        let graph = self.state.graph.read();
        let snapshot = self.state.snapshot.read();

        // Upstream (callers/dependents): transitive via reverse_deps.
        let mut callers: std::collections::HashSet<rpg_encoder::NodeId> =
            std::collections::HashSet::new();
        let mut frontier: Vec<(rpg_encoder::NodeId, usize)> =
            start_ids.iter().map(|&id| (id, 0)).collect();
        let mut visited = start_ids.iter().copied().collect::<std::collections::HashSet<_>>();
        while let Some((id, d)) = frontier.pop() {
            if d >= depth {
                continue;
            }
            for dep in snapshot.dependents_of(id) {
                if visited.insert(dep) {
                    callers.insert(dep);
                    frontier.push((dep, d + 1));
                }
            }
        }

        // Downstream (callees/dependencies): transitive over outgoing edges
        // of dependency types (Calls, UsesType, References, Extends, Implements).
        let dep_types = [
            rpg_encoder::EdgeType::Calls,
            rpg_encoder::EdgeType::UsesType,
            rpg_encoder::EdgeType::References,
            rpg_encoder::EdgeType::Extends,
            rpg_encoder::EdgeType::Implements,
        ];
        let mut callees: std::collections::HashSet<rpg_encoder::NodeId> =
            std::collections::HashSet::new();
        let mut frontier2: Vec<(rpg_encoder::NodeId, usize)> =
            start_ids.iter().map(|&id| (id, 0)).collect();
        let mut visited2 = start_ids.iter().copied().collect::<std::collections::HashSet<_>>();
        while let Some((id, d)) = frontier2.pop() {
            if d >= depth {
                continue;
            }
            for (nbr, edge) in graph.edges_from(id) {
                if !dep_types.contains(&edge.edge_type) {
                    continue;
                }
                if visited2.insert(nbr) {
                    callees.insert(nbr);
                    frontier2.push((nbr, d + 1));
                }
            }
        }

        // Affected files: the files containing all touched nodes.
        let touched: Vec<rpg_encoder::NodeId> = start_ids
            .iter()
            .chain(&callers)
            .chain(&callees)
            .copied()
            .collect();
        let affected_files: std::collections::HashSet<String> = touched
            .iter()
            .filter_map(|&id| graph.get_node(id))
            .filter_map(|n| n.path.as_ref())
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        let shape = |ids: &std::collections::HashSet<rpg_encoder::NodeId>| -> Vec<Value> {
            ids.iter()
                .filter_map(|&id| graph.get_node(id))
                .map(|n| {
                    json!({
                        "id": n.id.index(),
                        "name": n.name,
                        "kind": n.kind,
                        "path": n.path,
                    })
                })
                .collect()
        };

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "callers": shape(&callers),
                "callees": shape(&callees),
                "affected_files": affected_files,
                "caller_count": callers.len(),
                "callee_count": callees.len(),
                "affected_file_count": affected_files.len(),
            })
            .to_string(),
        )]))
    }

    /// Find potentially dead code: definitions with no incoming Calls or
    /// References edges.
    ///
    /// Returns functions/types/fields that nothing calls or references. These
    /// are candidates for removal (always review — entry points, FFI exports,
    /// and trait impls may legitimately have no in-graph callers).
    ///
    /// Params:
    /// - `scope`: optional path prefix to limit the scan (e.g. "src/legacy/")
    /// - `include_exported`: if true, don't filter out FFI exports (default false)
    #[tool(
        description = "Find potentially dead code: definitions with no incoming Calls/References edges. Candidates for removal. Params: scope (optional path prefix), include_exported (bool, default false — excludes FFI exports)."
    )]
    pub async fn find_dead_code(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let scope = get_str(&params, "scope");
        let limit = get_u64(&params, "limit").unwrap_or(100).min(MAX_RESULTS as u64) as usize;
        let include_exported = get_str(&params, "include_exported")
            .map(|s| s == "true" || s == "1")
            .unwrap_or(false);

        let graph = self.state.graph.read();

        // FFI export node ids — these are entry points that legitimately have
        // no in-graph callers, so exclude them unless explicitly requested.
        let ffi_exports: HashSet<NodeId> = if include_exported {
            HashSet::new()
        } else {
            graph
                .nodes()
                .filter(|n| {
                    // FFI bindings carry an FfiBinding edge or a no_mangle marker.
                    n.metadata.contains_key("ffi_kind")
                        || n.metadata.contains_key("export_name")
                })
                .map(|n| n.id)
                .collect()
        };

        let dead: Vec<Value> = graph
            .nodes()
            .filter(|n| crate::tools::format::is_definition(n.category))
            .filter(|n| {
                // Optional scope filter: node path must start with the prefix.
                if let Some(s) = scope {
                    n.path
                        .as_ref()
                        .map(|p| p.starts_with(s))
                        .unwrap_or(false)
                } else {
                    true
                }
            })
            .filter(|n| !ffi_exports.contains(&n.id))
            .filter(|n| {
                // Dead = no incoming Calls or References edges.
                // Use the non-allocating short-circuit check.
                !graph.has_incoming_of_types(n.id, &[EdgeType::Calls, EdgeType::References])
            })
            .map(|n| {
                json!({
                    "id": n.id.index(),
                    "name": n.name,
                    "kind": n.kind,
                    "category": n.category.to_string(),
                    "path": n.path,
                    "signature": n.signature,
                    "semantic_feature": n.semantic_feature,
                })
            })
            .take(limit)
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "dead_code": dead,
                "count": dead.len(),
                "limit": limit,
                "scope": scope,
                "note": "Review before removal — entry points, trait impls, and dynamically-called code may lack in-graph callers.",
            })
            .to_string(),
        )]))
    }

    #[tool(
        description = "Get the skeleton: file nodes with their direct children (Contains edges)"
    )]
    pub async fn get_skeleton(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let limit = get_u64(&params, "limit").unwrap_or(500).min(MAX_RESULTS as u64) as usize;
        let graph = self.state.graph.read();

        let file_nodes: Vec<_> = graph
            .nodes()
            .filter(|n| n.category == NodeCategory::File)
            .collect();

        let mut skeleton: Vec<Value> = Vec::new();

        for file_node in &file_nodes {
            let file_id = file_node.id;
            let mut children: Vec<Value> = Vec::new();

            // Use edges_from (O(degree)) instead of edges() (O(total edges)).
            // The old code was O(F × E) — at 10k nodes that's ~100M iterations.
            for (tgt, edge) in graph.edges_from(file_id) {
                if edge.edge_type == EdgeType::Contains {
                    if let Some(child) = graph.get_node(tgt) {
                        children.push(json!({
                            "id": child.id.index(),
                            "name": child.name,
                            "kind": child.kind,
                            "category": child.category.to_string(),
                        }));
                    }
                }
            }

            skeleton.push(json!({
                "id": file_node.id.index(),
                "name": file_node.name,
                "path": file_node.path,
                "language": file_node.language,
                "children": children,
                "child_count": children.len(),
            }));
        }

        skeleton.sort_by(|a, b| {
            a["path"]
                .as_str()
                .unwrap_or("")
                .cmp(b["path"].as_str().unwrap_or(""))
        });
        skeleton.truncate(limit);

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "files": skeleton,
                "total_files": skeleton.len(),
                "limit": limit,
            })
            .to_string(),
        )]))
    }

    #[tool(
        description = "Get nodes with features or descriptions, optionally filtered by file path"
    )]
    pub async fn get_features(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let file_path = get_str(&params, "file_path");
        let limit = get_u64(&params, "limit").unwrap_or(100).min(500) as usize;

        let graph = self.state.graph.read();
        let mut results: Vec<Value> = Vec::new();

        for node in graph.nodes() {
            let has_features = !node.features.is_empty();
            let has_description = node.description.as_ref().is_some_and(|d| !d.is_empty());

            if !has_features && !has_description {
                continue;
            }
            if let Some(fp) = file_path {
                if node
                    .path
                    .as_ref()
                    .is_none_or(|p| !p.to_string_lossy().contains(fp))
                {
                    continue;
                }
            }

            results.push(json!({
                "id": node.id.index(),
                "name": node.name,
                "kind": node.kind,
                "category": node.category.to_string(),
                "path": node.path,
                "features": node.features,
                "feature_path": node.feature_path,
                "description": node.description,
            }));

            if results.len() >= limit {
                break;
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({ "nodes": results, "count": results.len() }).to_string(),
        )]))
    }

    #[tool(description = "Get nodes with Component category")]
    pub async fn get_components(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let limit = get_u64(&params, "limit").unwrap_or(100).min(500) as usize;

        let graph = self.state.graph.read();
        let mut results: Vec<Value> = Vec::new();

        for node in graph.nodes() {
            if node.category != NodeCategory::Component {
                continue;
            }

            results.push(json!({
                "id": node.id.index(),
                "name": node.name,
                "kind": node.kind,
                "language": node.language,
                "path": node.path,
                "description": node.description,
                "features": node.features,
            }));

            if results.len() >= limit {
                break;
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({ "components": results, "count": results.len() }).to_string(),
        )]))
    }

    #[tool(
        description = "Browse the functional (V^H) hierarchy: high-level feature centroids and their member nodes. Use to understand what a repo does at a behavioral level. Returns each centroid with its member functions/types."
    )]
    pub async fn get_feature_tree(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let limit = get_u64(&params, "limit").unwrap_or(50).min(500) as usize;

        let graph = self.state.graph.read();

        let centroids: Vec<Value> = graph
            .functional_centroids()
            .take(limit)
            .map(|c| {
                let members: Vec<Value> = graph
                    .centroid_members(c.id)
                    .into_iter()
                    .map(|m| {
                        json!({
                            "id": m.id.index(),
                            "name": m.name,
                            "kind": m.kind,
                            "path": m.path,
                        })
                    })
                    .collect();
                json!({
                    "id": c.id.index(),
                    "name": c.name,
                    "description": c.description,
                    "semantic_feature": c.semantic_feature,
                    "member_count": members.len(),
                    "members": members,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "centroids": centroids,
                "centroid_count": centroids.len(),
            })
            .to_string(),
        )]))
    }

    #[tool(
        description = "List FFI (foreign function interface) bindings — cross-language edges unique to rpg-mcp. Filter by language (e.g. 'rust') or kind ('export'/'import'). Shows what native functions a language binds to."
    )]
    pub async fn get_ffi_bindings(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let lang_filter = get_str(&params, "language");
        let kind_filter = get_str(&params, "kind");
        let limit = get_u64(&params, "limit").unwrap_or(100).min(500) as usize;

        let graph = self.state.graph.read();

        // FFI binding nodes are NodeCategory::Feature, kind "ffi_binding".
        // The owning file connects via an FfiBinding edge carrying metadata.
        let mut results: Vec<Value> = Vec::new();

        for node in graph.nodes() {
            if node.kind != "ffi_binding" {
                continue;
            }
            if results.len() >= limit {
                break;
            }

            // Find the owning file via incoming FfiBinding edges.
            let mut owner_file = None;
            let mut ffi_meta = serde_json::Map::new();
            for (src, edge) in graph.edges_to(node.id) {
                if edge.edge_type == rpg_encoder::EdgeType::FfiBinding {
                    if let Some(owner) = graph.get_node(src) {
                        owner_file = Some(owner.name.clone());
                    }
                    // Merge edge metadata (ffi_source, ffi_target, ffi_kind, ffi_signature).
                    for (k, v) in &edge.metadata {
                        ffi_meta.insert(k.clone(), v.clone());
                    }
                }
            }

            let source_lang = ffi_meta
                .get("ffi_source")
                .and_then(|v| v.as_str())
                .unwrap_or(&node.language);
            let ffi_kind = ffi_meta
                .get("ffi_kind")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            // Apply filters.
            if let Some(lf) = lang_filter {
                if source_lang != lf {
                    continue;
                }
            }
            if let Some(kf) = kind_filter {
                if !ffi_kind.contains(kf) {
                    continue;
                }
            }

            results.push(json!({
                "symbol": node.name,
                "source_lang": source_lang,
                "target_lang": ffi_meta.get("ffi_target").and_then(|v| v.as_str()),
                "kind": ffi_kind,
                "owner_file": owner_file,
                "signature": ffi_meta.get("ffi_signature").and_then(|v| v.as_str()),
                "node_id": node.id.index(),
            }));
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({ "ffi_bindings": results, "count": results.len() }).to_string(),
        )]))
    }

    #[tool(
        description = "Semantic search over LLM-extracted behavioral features. Find nodes by WHAT THEY DO (features, description, feature_path), not just by name. This is rpg-mcp's key differentiator: searches the semantic enrichment no competitor has. Requires prior semantic encoding (RPG_SEMANTIC=true)."
    )]
    pub async fn semantic_search(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let query = get_str(&params, "query")
            .ok_or_else(|| McpError::invalid_params("missing 'query'", None))?;
        let top_k = get_u64(&params, "top_k").unwrap_or(10).min(500) as usize;
        let scope = get_str(&params, "scope"); // optional feature_path prefix

        let query_lower = query.to_ascii_lowercase();
        let query_terms: Vec<&str> = query_lower.split_whitespace().collect();

        let graph = self.state.graph.read();

        // Score each node by token overlap with its semantic fields.
        let mut scored: Vec<(f64, &rpg_encoder::Node)> = graph
            .nodes()
            .filter_map(|node| {
                // Only consider nodes that have semantic enrichment.
                let has_semantics = node.semantic_feature.is_some()
                    || !node.features.is_empty()
                    || node.description.is_some();
                if !has_semantics {
                    return None;
                }

                // Optional feature_path scope filter.
                if let Some(s) = scope {
                    if let Some(ref fp) = node.feature_path {
                        if !fp.to_ascii_lowercase().starts_with(&s.to_ascii_lowercase()) {
                            return None;
                        }
                    } else {
                        return None;
                    }
                }

                // Build the searchable text from semantic fields.
                let mut text = String::new();
                if let Some(ref sf) = node.semantic_feature {
                    text.push_str(sf);
                    text.push(' ');
                }
                for f in &node.features {
                    text.push_str(f);
                    text.push(' ');
                }
                if let Some(ref desc) = node.description {
                    text.push_str(desc);
                    text.push(' ');
                }
                if let Some(ref fp) = node.feature_path {
                    text.push_str(fp);
                }
                let text_lower = text.to_ascii_lowercase();
                let node_name_lower = node.name.to_ascii_lowercase();

                // Score: count how many query terms appear in the text, with
                // a bonus for exact name match. Simple but effective for
                // behavioral search ("user authentication", "parse config").
                let mut score = 0.0f64;
                for term in &query_terms {
                    if text_lower.contains(term) {
                        score += 1.0;
                    }
                    if node_name_lower.contains(term) {
                        score += 0.5; // name match is a strong signal
                    }
                }
                // Normalize by query length to favor multi-term matches.
                if !query_terms.is_empty() {
                    score /= query_terms.len() as f64;
                }

                if score > 0.0 {
                    Some((score, node))
                } else {
                    None
                }
            })
            .collect();

        // Sort by score descending, take top_k.
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        let results: Vec<Value> = scored
            .into_iter()
            .map(|(score, n)| {
                json!({
                    "id": n.id.index(),
                    "name": n.name,
                    "kind": n.kind,
                    "category": n.category.to_string(),
                    "path": n.path,
                    "score": (score * 100.0).round() / 100.0,
                    "features": n.features,
                    "description": n.description,
                    "feature_path": n.feature_path,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "results": results,
                "count": results.len(),
                "query": query,
            })
            .to_string(),
        )]))
    }

    /// Find the definition node enclosing a given file:line.
    ///
    /// Scans nodes whose `path` matches and whose `[start_line, end_line]`
    /// range contains the query line. Returns the tightest (innermost)
    /// enclosing definition, with its parent (the next-wider enclosing node)
    /// if one exists. Useful for "what is the cursor inside?" from an editor.
    #[tool(
        description = "Find the definition node enclosing a file:line. Returns the innermost enclosing node (function/type/etc.) plus its parent. Useful for jump-to-def and 'what is my cursor inside?'. Params: file (path), line (1-based)."
    )]
    pub async fn find_node_at_location(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let file = get_str(&params, "file")
            .ok_or_else(|| McpError::invalid_params("missing 'file'", None))?;
        let line = get_u64(&params, "line")
            .ok_or_else(|| McpError::invalid_params("missing 'line'", None))? as usize;
        if line == 0 {
            return Err(McpError::invalid_params("'line' is 1-based and must be > 0", None));
        }

        let graph = self.state.graph.read();
        let target = Path::new(file);

        // Collect all nodes whose path matches and whose line range contains
        // the query line. Prefer source_ref (line range), fall back to location.
        let mut enclosing: Vec<&rpg_encoder::Node> = graph
            .nodes()
            .filter(|n| {
                if n.path.as_deref() != Some(target) {
                    return false;
                }
                let (start, end) = n
                    .source_ref
                    .as_ref()
                    .map(|sr| (sr.start_line, sr.end_line))
                    .or_else(|| {
                        n.location.as_ref().map(|l| (l.start_line, l.end_line))
                    })
                    .unwrap_or((0, 0));
                start > 0 && line >= start && line <= end
            })
            .collect();

        if enclosing.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                json!({
                    "found": false,
                    "file": file,
                    "line": line,
                })
                .to_string(),
            )]));
        }

        // Innermost = smallest span. Sort by span ascending; first is tightest.
        enclosing.sort_by_key(|n| {
            let (s, e) = n
                .source_ref
                .as_ref()
                .map(|sr| (sr.start_line, sr.end_line))
                .or_else(|| n.location.as_ref().map(|l| (l.start_line, l.end_line)))
                .unwrap_or((0, 0));
            e.saturating_sub(s)
        });

        let inner = enclosing[0];
        // Parent = the next-wider enclosing node (largest span that still
        // contains the inner node's start, excluding the inner itself).
        let parent = enclosing
            .iter()
            .skip(1)
            .copied()
            .max_by_key(|n| {
                let (s, e) = n
                    .source_ref
                    .as_ref()
                    .map(|sr| (sr.start_line, sr.end_line))
                    .or_else(|| n.location.as_ref().map(|l| (l.start_line, l.end_line)))
                    .unwrap_or((0, 0));
                e.saturating_sub(s)
            });

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "found": true,
                "file": file,
                "line": line,
                "node": node_to_json(inner, crate::tools::format::DetailLevel::Summary),
                "parent": parent.map(|p| node_to_json(p, crate::tools::format::DetailLevel::Minimal)),
            })
            .to_string(),
        )]))
    }

    /// Repo-level architecture overview: hub nodes, centroid distribution,
    /// language breakdown, and largest files.
    ///
    /// Synthesizes a quick "you are here" map of the codebase. Hub nodes are
    /// the most-depended-on definitions (top-N by incoming edge count).
    /// Centroid distribution shows the functional areas (V^H) and how many
    /// low-level nodes each aggregates.
    #[tool(
        description = "Repo-level architecture overview: hub nodes (most-depended-on), functional centroid distribution, language breakdown, largest files. Use for 'give me the lay of the land' after encode."
    )]
    pub async fn get_architecture_overview(
        &self,
        params: JsonObject,
    ) -> Result<CallToolResult, McpError> {
        let top_n = get_u64(&params, "top_n").unwrap_or(10).min(500) as usize;

        let graph = self.state.graph.read();

        // Hub nodes: definitions with the most incoming edges.
        let mut hubs: Vec<(NodeId, usize, &rpg_encoder::Node)> = graph
            .nodes()
            .filter(|n| crate::tools::format::is_definition(n.category))
            .map(|n| {
                // Use the non-allocating in_degree() instead of edges_to().len().
                let incoming = graph.in_degree(n.id);
                (n.id, incoming, n)
            })
            .filter(|(_, cnt, _)| *cnt > 0)
            .collect();
        hubs.sort_unstable_by_key(|(_, cnt, _)| std::cmp::Reverse(*cnt));
        let hubs_json: Vec<Value> = hubs
            .iter()
            .take(top_n)
            .map(|(id, cnt, n)| {
                json!({
                    "id": id.index(),
                    "name": n.name,
                    "kind": n.kind,
                    "category": n.category.to_string(),
                    "path": n.path,
                    "incoming_edges": cnt,
                })
            })
            .collect();

        // Centroid distribution: functional areas + member counts.
        let centroids: Vec<Value> = graph
            .functional_centroids()
            .map(|c| {
                let member_count = graph.centroid_members(c.id).len();
                json!({
                    "id": c.id.index(),
                    "name": c.name,
                    "description": c.description,
                    "members": member_count,
                })
            })
            .collect();

        // Language breakdown.
        let mut lang_counts: HashMap<String, usize> = HashMap::new();
        let mut file_count = 0usize;
        for node in graph.nodes() {
            if !node.language.is_empty() {
                *lang_counts.entry(node.language.clone()).or_default() += 1;
            }
            if node.category == NodeCategory::File {
                file_count += 1;
            }
        }
        let mut languages: Vec<(String, usize)> = lang_counts.into_iter().collect();
        languages.sort_unstable_by_key(|(_, c)| std::cmp::Reverse(*c));

        // Largest files by node count.
        let mut per_file: HashMap<PathBuf, usize> = HashMap::new();
        for node in graph.nodes() {
            if let Some(ref p) = node.path {
                *per_file.entry(p.clone()).or_default() += 1;
            }
        }
        let mut files: Vec<(PathBuf, usize)> = per_file.into_iter().collect();
        files.sort_unstable_by_key(|(_, c)| std::cmp::Reverse(*c));
        let largest_files: Vec<Value> = files
            .iter()
            .take(top_n)
            .map(|(p, c)| {
                json!({
                    "path": p,
                    "nodes": c,
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "total_nodes": graph.node_count(),
                "total_edges": graph.edge_count(),
                "file_nodes": file_count,
                "hub_nodes": hubs_json,
                "functional_areas": centroids,
                "languages": languages.iter().map(|(l, c)| json!({"language": l, "nodes": c})).collect::<Vec<_>>(),
                "largest_files": largest_files,
            })
            .to_string(),
        )]))
    }

    /// Vector (embedding) semantic search over node text.
    ///
    /// Embeds the query via the configured endpoint (Qwen3-Embedding), then
    /// cosine-searches the sidecar index for the top-k node ids. Falls back to
    /// the keyword-based `semantic_search` when no index is available.
    ///
    /// Requires a prior `encode_repo` with embeddings enabled (i.e. the file
    /// `<workspace>/.rpg/embeddings.bin` exists).
    #[tool(
        description = "Vector (embedding) search: finds nodes by semantic similarity to a natural-language query. Embeds the query and cosine-searches the embedding index. Stronger than keyword search for 'find code that does X'. Requires prior encoding with embeddings enabled."
    )]
    pub async fn vector_search(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let _tel = crate::tools::telemetry::timed(&self.telemetry, "vector_search", &params);
        let query = get_str(&params, "query")
            .ok_or_else(|| McpError::invalid_params("missing 'query'", None))?;
        let top_k = get_u64(&params, "top_k").unwrap_or(10).min(500) as usize;
        let detail = get_str(&params, "detail_level").unwrap_or("summary");

        // Lazily load the sidecar index on first use.
        let sidecar = self.state.config.data_dir.join("embeddings.bin");
        {
            let emb_guard = self.state.embeddings.read();
            if emb_guard.is_none() && !sidecar.exists() {
                return Ok(CallToolResult::success(vec![Content::text(
                    json!({
                        "status": "unavailable",
                        "error": "no embedding index found — run encode_repo with embeddings enabled first",
                        "fallback": "use the semantic_search tool for keyword-based search",
                    })
                    .to_string(),
                )]));
            }
        }
        if self.state.embeddings.read().is_none() {
            match rpg_encoder::FlatIndex::load(&sidecar) {
                Ok(idx) => {
                    *self.state.embeddings.write() = Some(idx);
                }
                Err(e) => {
                    return Ok(CallToolResult::success(vec![Content::text(
                        json!({
                            "status": "error",
                            "error": format!("failed to load embedding index: {e}"),
                        })
                        .to_string(),
                    )]));
                }
            }
        }

        // Embed the query + search. The embed HTTP call must happen without any
        // lock guard held (it's async + the guard isn't Send); the index search
        // itself is synchronous and takes the read lock only for that call.
        let emb_config = rpg_encoder::EmbeddingConfig::from_env();
        let client = rpg_encoder::EmbeddingClient::new(emb_config.clone())
            .map_err(|e| McpError::internal_error(format!("Embedding client init: {e}"), None))?;

        // Embedding the query hits the network; a failure here is an expected
        // runtime condition (endpoint down), so return a JSON error body.
        let qv = match client.embed_batch(&[query.to_string()]).await {
            Ok(v) => v,
            Err(e) => {
                return Ok(CallToolResult::success(vec![Content::text(
                    json!({
                        "status": "error",
                        "error": format!("failed to embed query: {e}"),
                        "endpoint": emb_config.endpoint,
                    })
                    .to_string(),
                )]));
            }
        };
        let qv = qv.into_iter().next().ok_or_else(|| {
            McpError::internal_error("empty embedding response", None)
        })?;

        let results: Vec<(rpg_encoder::NodeId, f32)> = {
            let emb_guard = self.state.embeddings.read();
            let idx = emb_guard.as_ref().expect("index just loaded");
            idx.search(&qv, top_k)
        };

        // Hydrate results with node data from the graph.
        let graph = self.state.graph.read();
        let level = parse_detail_level(detail);
        let hits: Vec<Value> = results
            .into_iter()
            .filter_map(|(id, score)| {
                graph.get_node(id).map(|node| {
                    json!({
                        "id": node.id.index(),
                        "name": node.name,
                        "kind": node.kind,
                        "category": node.category.to_string(),
                        "path": node.path,
                        "score": (score * 1000.0).round() / 1000.0,
                        "semantic_feature": node.semantic_feature,
                        "description": node.description,
                        "features": node.features,
                        "node_level": node.node_level.to_string(),
                        "_detail": format!("{level:?}").to_ascii_lowercase(),
                    })
                })
            })
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "results": hits,
                "count": hits.len(),
                "query": query,
                "backend": "flat",
            })
            .to_string(),
        )]))
    }

    /// Analyze a unified diff against the graph: which nodes changed, what's
    /// the impact (callers/callees of changed code), affected files, and a
    /// simple risk score.
    ///
    /// The risk score for each changed node is based on fan-out: nodes with
    /// high incoming edge counts (many dependents) that also have a high change
    /// ratio (lines changed / total lines) get the highest scores.
    #[tool(
        description = "Analyze a unified diff against the graph: changed nodes, impact (callers/callees), affected files, risk scores. Params: diff (unified diff string), depth (impact traversal depth, default 2)."
    )]
    pub async fn analyze_diff(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let diff_text = get_str(&params, "diff")
            .ok_or_else(|| McpError::invalid_params("missing 'diff'", None))?;
        let depth = get_u64(&params, "depth").unwrap_or(2).clamp(1, 10) as usize;

        let parsed = crate::tools::diff::ParsedDiff::parse(diff_text);
        if parsed.files.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                json!({ "changed_nodes": [], "count": 0, "note": "no changes found in diff" })
                    .to_string(),
            )]));
        }

        let graph = self.state.graph.read();
        let mut changed_nodes: Vec<Value> = Vec::new();
        let mut all_affected: HashSet<NodeId> = HashSet::new();

        for (file, ranges) in &parsed.files {
            let path = Path::new(file);
            // Find nodes in this file whose source range overlaps any changed range.
            for node in graph.nodes_for_file(path) {
                let (node_start, node_end) = node
                    .source_ref
                    .as_ref()
                    .map(|sr| (sr.start_line, sr.end_line))
                    .or_else(|| node.location.as_ref().map(|l| (l.start_line, l.end_line)))
                    .unwrap_or((0, 0));
                if node_start == 0 {
                    continue;
                }

                // Check if any changed range overlaps the node's line range.
                let overlaps = ranges.iter().any(|r| {
                    !(r.end < node_start || r.start > node_end)
                });
                if !overlaps {
                    continue;
                }

                // Count incoming edges for risk scoring (non-allocating).
                let incoming = graph.in_degree(node.id);
                let total_lines = node_end.saturating_sub(node_start).max(1);
                let changed_lines: usize = ranges
                    .iter()
                    .filter(|r| !(r.end < node_start || r.start > node_end))
                    .map(|r| {
                        let lo = r.start.max(node_start);
                        let hi = r.end.min(node_end);
                        hi.saturating_sub(lo) + 1
                    })
                    .sum::<usize>().max(1);
                let change_ratio = changed_lines as f64 / total_lines as f64;
                // Risk: 0-10 scale. incoming_edges * change_ratio, scaled.
                let risk = (incoming as f64 * change_ratio * 2.0).min(10.0);

                changed_nodes.push(json!({
                    "id": node.id.index(),
                    "name": node.name,
                    "kind": node.kind,
                    "category": node.category.to_string(),
                    "path": node.path,
                    "incoming_edges": incoming,
                    "lines_changed": changed_lines,
                    "total_lines": total_lines,
                    "change_ratio": (change_ratio * 100.0).round() / 100.0,
                    "risk_score": (risk * 10.0).round() / 10.0,
                }));

                // Collect impact: callers + callees within `depth` hops.
                let callers = bfs_node_ids(&graph, node.id, true, depth);
                let callees = bfs_node_ids(&graph, node.id, false, depth);
                all_affected.extend(callers);
                all_affected.extend(callees);
                all_affected.insert(node.id);
            }
        }

        // Affected files: unique paths from all affected nodes.
        let affected_files: HashSet<&PathBuf> = all_affected
            .iter()
            .filter_map(|id| graph.get_node(*id))
            .filter_map(|n| n.path.as_ref())
            .collect();

        let risk_nodes: Vec<&Value> = changed_nodes
            .iter()
            .filter(|v| v["risk_score"].as_f64().unwrap_or(0.0) >= 5.0)
            .collect();

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "changed_nodes": changed_nodes,
                "changed_count": changed_nodes.len(),
                "affected_nodes": all_affected.len(),
                "affected_files": affected_files.len(),
                "high_risk_count": risk_nodes.len(),
                "impact_depth": depth,
            })
            .to_string(),
        )]))
    }

    /// Get the nodes that changed since a git ref (or in the working tree).
    #[tool(
        description = "Get nodes changed since a git ref. Params: since (optional git ref like HEAD~1 or main; defaults to working-tree diff). Returns changed node ids + names."
    )]
    pub async fn get_changed_nodes(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let since = get_str(&params, "since");
        let workspace = self.state.config.workspace.clone();

        // Run git diff to get the unified diff.
        let diff_output = match since {
            Some(ref_arg) => std::process::Command::new("git")
                .args(["diff", ref_arg])
                .current_dir(&workspace)
                .output()
                .map_err(|e| McpError::internal_error(format!("git diff failed: {e}"), None))?,
            None => std::process::Command::new("git")
                .args(["diff"])
                .current_dir(&workspace)
                .output()
                .map_err(|e| McpError::internal_error(format!("git diff failed: {e}"), None))?,
        };

        if !diff_output.status.success() {
            return Ok(CallToolResult::success(vec![Content::text(
                json!({
                    "error": String::from_utf8_lossy(&diff_output.stderr).to_string(),
                    "note": "git diff failed — is this a git repo?",
                })
                .to_string(),
            )]));
        }

        let diff_text = String::from_utf8_lossy(&diff_output.stdout);
        let parsed = crate::tools::diff::ParsedDiff::parse(&diff_text);

        let graph = self.state.graph.read();
        let mut changed: Vec<Value> = Vec::new();
        for (file, ranges) in &parsed.files {
            let path = Path::new(file);
            for node in graph.nodes_for_file(path) {
                let (node_start, node_end) = node
                    .source_ref
                    .as_ref()
                    .map(|sr| (sr.start_line, sr.end_line))
                    .or_else(|| node.location.as_ref().map(|l| (l.start_line, l.end_line)))
                    .unwrap_or((0, 0));
                if node_start == 0 {
                    continue;
                }
                let overlaps = ranges.iter().any(|r| !(r.end < node_start || r.start > node_end));
                if overlaps {
                    changed.push(json!({
                        "id": node.id.index(),
                        "name": node.name,
                        "kind": node.kind,
                        "path": node.path,
                    }));
                }
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "changed_nodes": changed,
                "count": changed.len(),
                "since": since,
            })
            .to_string(),
        )]))
    }

    /// Write a memory (cross-session note) tied to a node or file.
    #[tool(
        description = "Write a cross-session memory note. Params: content (required), node_id (optional), file (optional path), tags (optional array of strings). Returns the memory id."
    )]
    pub async fn write_memory(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let content = get_str(&params, "content")
            .ok_or_else(|| McpError::invalid_params("missing 'content'", None))?
            .to_string();
        let node_id = get_u64(&params, "node_id");
        let file = get_str(&params, "file").map(String::from);
        let tags: Vec<String> = params
            .get("tags")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let id = self.state.memories.write(content, node_id, file, tags);

        Ok(CallToolResult::success(vec![Content::text(
            json!({ "id": id, "status": "ok" }).to_string(),
        )]))
    }

    /// Read a memory by id.
    #[tool(description = "Read a memory by id. Params: id (required).")]
    pub async fn read_memory(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let id = get_str(&params, "id")
            .ok_or_else(|| McpError::invalid_params("missing 'id'", None))?;
        match self.state.memories.read(id) {
            Some(mem) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string(&mem).unwrap_or_default(),
            )])),
            None => Ok(CallToolResult::success(vec![Content::text(
                json!({ "found": false, "id": id }).to_string(),
            )])),
        }
    }

    /// List memories, optionally filtered by node_id, file, or tag.
    #[tool(
        description = "List memories with optional filters. Params: node_id, file, tag (all optional). Returns matching memories."
    )]
    pub async fn list_memories(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let node_id = get_u64(&params, "node_id");
        let file = get_str(&params, "file");
        let tag = get_str(&params, "tag");

        let memories = self.state.memories.list(node_id, file, tag);
        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "memories": memories,
                "count": memories.len(),
            })
            .to_string(),
        )]))
    }

    /// Delete a memory by id.
    #[tool(description = "Delete a memory by id. Params: id (required).")]
    pub async fn delete_memory(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let id = get_str(&params, "id")
            .ok_or_else(|| McpError::invalid_params("missing 'id'", None))?;
        let deleted = self.state.memories.delete(id);
        Ok(CallToolResult::success(vec![Content::text(
            json!({ "deleted": deleted, "id": id }).to_string(),
        )]))
    }

    /// Export the graph in a standard format for visualization / external tools.
    ///
    /// Formats: `graphml` (Gephi/yEd/Cytoscape), `cypher` (Neo4j), `dot`
    /// (Graphviz). The hierarchy (Contains edges) is preserved in all formats.
    #[tool(
        description = "Export the graph in GraphML/Cypher/DOT format for external tools (Gephi, Neo4j, Graphviz). Params: format (graphml|cypher|dot), path (optional file to write; if omitted returns the serialized string)."
    )]
    pub async fn export_graph(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let format = get_str(&params, "format").unwrap_or("graphml");
        let path = get_str(&params, "path");

        let graph = self.state.graph.read();
        let output = match format.to_ascii_lowercase().as_str() {
            "graphml" => rpg_encoder::to_graphml(&graph),
            "cypher" => rpg_encoder::to_cypher(&graph),
            "dot" => rpg_encoder::to_dot(&graph),
            other => {
                return Err(McpError::invalid_params(
                    format!("unknown format '{other}' — use graphml, cypher, or dot"),
                    None,
                ));
            }
        };
        let node_count = graph.node_count();
        let edge_count = graph.edge_count();
        drop(graph);

        // Optionally write to a file.
        if let Some(path) = path {
            std::fs::write(path, &output)
                .map_err(|e| McpError::internal_error(format!("write failed: {e}"), None))?;
        }

        // Size guard: if the output is large and no file path was given,
        // return only a summary to avoid MCP protocol bloat.
        const MAX_INLINE_OUTPUT: usize = 100_000; // 100KB
        let (output_field, truncated) = if output.len() > MAX_INLINE_OUTPUT && path.is_none() {
            (output[..MAX_INLINE_OUTPUT.min(output.len())].to_string(), true)
        } else {
            (output.clone(), false)
        };

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "format": format,
                "nodes": node_count,
                "edges": edge_count,
                "bytes": output.len(),
                "path": path,
                "truncated": truncated,
                "output": output_field,
            })
            .to_string(),
        )]))
    }

    /// Enrich the graph's edges with precise SCIP (Sourcegraph Code Intelligence
    /// Protocol) relationship data.
    ///
    /// Tree-sitter produces structural nodes + heuristic name-based edges; SCIP
    /// provides compiler-grade symbol resolution. This pass rewrites the weak
    /// Calls/References/UsesType edges to precise ones where SCIP data is
    /// available. SCIP-sourced edges are marked with `metadata.source = "scip"`.
    #[tool(
        description = "Enrich graph edges with SCIP (compiler-grade) symbol resolution. Params: scip_file (path to .scip index), or scip_data (inline JSON array of occurrences + relationships)."
    )]
    pub async fn enrich_with_scip(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        use rpg_encoder::{enrich_graph_scip, ScipIndex};

        // Load the SCIP index from inline JSON (the struct deserializes from
        // { occurrences: [...], relationships: [...] }) or from a JSON file.
        let index = if let Some(data) = params.get("scip_data") {
            serde_json::from_value::<ScipIndex>(data.clone())
                .map_err(|e| McpError::invalid_params(format!("invalid scip_data: {e}"), None))?
        } else if let Some(file) = get_str(&params, "scip_file") {
            // Read the file as JSON for now (the scip-parse feature adds
            // protobuf parsing). This covers the common test/dev case.
            let json_str = std::fs::read_to_string(file)
                .map_err(|e| McpError::internal_error(format!("read scip_file: {e}"), None))?;
            serde_json::from_str::<ScipIndex>(&json_str)
                .map_err(|e| McpError::invalid_params(format!("parse scip_file as JSON: {e}"), None))?
        } else {
            return Err(McpError::invalid_params(
                "missing 'scip_file' or 'scip_data'",
                None,
            ));
        };

        let mut graph = self.state.graph.write();
        let stats = enrich_graph_scip(&mut graph, &index);

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "symbols_mapped": stats.symbols_mapped,
                "symbols_unmapped": stats.symbols_unmapped,
                "edges_added": stats.edges_added,
                "edges_confirmed": stats.edges_confirmed,
            })
            .to_string(),
        )]))
    }
}

#[tool_handler]
impl ServerHandler for RpgService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::default(),
            server_info: Implementation {
                name: "rpg-mcp".to_string(),
                title: None,
                version: "0.1.0".to_string(),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "RPG MCP server for code graph analysis with semantic features, embeddings, and FFI support.\n\
                 \n\
                 GETTING STARTED:\n\
                 - encode_repo: build/rebuild the graph from the workspace\n\
                 - detect_changes: incrementally re-encode only changed files (faster than encode_repo)\n\
                 - get_graph_summary: overview of nodes, edges, languages, validation\n\
                 \n\
                 FINDING CODE:\n\
                 - search_nodes: find by name substring (structural)\n\
                 - semantic_search: find by WHAT CODE DOES (behavioral — keyword search over LLM-extracted features)\n\
                 - vector_search: embedding similarity search (stronger for 'find code that does X'). Requires encode_embeddings first\n\
                 - encode_embeddings: compute vector embeddings over graph nodes (run after a semantic encode)\n\
                 - get_node_details: full info for a node (pass detail_level=full for metadata/location)\n\
                 - find_node_at_location: given a file+line, find the enclosing definition\n\
                 \n\
                 NAVIGATING RELATIONSHIPS:\n\
                 - get_callers / get_callees: who calls X / what does X call (depth>1 for transitive)\n\
                 - explore_graph: BFS traversal in any direction, any depth, edge-type filtered\n\
                 - get_edges: direct edge query by source/target/type\n\
                 - get_impact: blast radius of changing a node (callers + callees + affected files)\n\
                 - get_source: read the actual source lines for a node\n\
                 - find_dead_code: definitions with no incoming calls/references (removal candidates)\n\
                 \n\
                 UNDERSTANDING ARCHITECTURE:\n\
                 - get_feature_tree: the functional hierarchy (behavioral areas + members)\n\
                 - get_features: nodes carrying LLM-extracted features\n\
                 - get_components: logical component groupings\n\
                 - get_skeleton: file → children structure\n\
                 - get_architecture_overview: hub nodes, centroid distribution, language breakdown\n\
                 \n\
                 CROSS-LANGUAGE / FFI:\n\
                 - get_ffi_bindings: what native functions a language binds to (unique to rpg-mcp)\n\
                 \n\
                 CHANGE ANALYSIS:\n\
                 - analyze_diff: graph-aware diff analysis with risk scoring\n\
                 - get_changed_nodes: which nodes changed since a git ref\n\
                 \n\
                 CODE INTELLIGENCE:\n\
                 - enrich_with_scip: refine edges with compiler-grade SCIP symbol resolution\n\
                 \n\
                 AGENT MEMORY:\n\
                 - write_memory / read_memory / list_memories / delete_memory: cross-session notes\n\
                 \n\
                 EXPORT:\n\
                 - export_graph: GraphML / Cypher / DOT format for external tools\n\
                 \n\
                 Tip: most tools accept detail_level (minimal/summary/full) and limit to control response size."
                    .to_string(),
            ),
        }
    }
}

/// BFS over all edge types from `start`, returning reachable NodeIds.
/// `upstream = true` follows incoming; `false` follows outgoing.
fn bfs_node_ids(
    graph: &rpg_encoder::RpgGraph,
    start: NodeId,
    upstream: bool,
    depth: usize,
) -> HashSet<NodeId> {
    let mut visited: HashSet<NodeId> = HashSet::new();
    visited.insert(start);
    let mut frontier: Vec<(NodeId, usize)> = vec![(start, 0)];

    while let Some((node_id, d)) = frontier.pop() {
        if d >= depth {
            continue;
        }
        let neighbors: Vec<(NodeId, &rpg_encoder::Edge)> = if upstream {
            graph.edges_to(node_id)
        } else {
            graph.edges_from(node_id)
        };
        for (nbr, _) in neighbors {
            if visited.insert(nbr) {
                frontier.push((nbr, d + 1));
            }
        }
    }
    visited
}

/// BFS over Calls edges from `start`, returning reachable node JSON objects.
/// `upstream = true` follows incoming edges (callers); `false` follows
/// outgoing edges (callees). Depth-bounded + limit-capped.
fn transitive_direction(
    graph: &rpg_encoder::RpgGraph,
    start: rpg_encoder::NodeId,
    depth: usize,
    limit: usize,
    upstream: bool,
) -> Vec<Value> {
    let mut visited: std::collections::HashSet<rpg_encoder::NodeId> =
        std::collections::HashSet::new();
    visited.insert(start);
    let mut frontier: Vec<(rpg_encoder::NodeId, usize)> = vec![(start, 0)];
    let mut out: Vec<Value> = Vec::new();

    while let Some((node_id, d)) = frontier.pop() {
        if d >= depth || out.len() >= limit {
            continue;
        }
        let neighbors: Vec<(rpg_encoder::NodeId, &rpg_encoder::Edge)> = if upstream {
            graph.edges_to(node_id)
        } else {
            graph.edges_from(node_id)
        };
        for (nbr, edge) in neighbors {
            if edge.edge_type != rpg_encoder::EdgeType::Calls {
                continue;
            }
            if visited.contains(&nbr) {
                continue;
            }
            visited.insert(nbr);
            if let Some(n) = graph.get_node(nbr) {
                out.push(json!({
                    "id": n.id.index(),
                    "name": n.name,
                    "kind": n.kind,
                    "path": n.path,
                    "depth": d + 1,
                }));
            }
            frontier.push((nbr, d + 1));
            if out.len() >= limit {
                break;
            }
        }
    }

    out
}

fn parse_category(s: &str) -> Option<NodeCategory> {
    match s.to_lowercase().as_str() {
        "repository" => Some(NodeCategory::Repository),
        "directory" => Some(NodeCategory::Directory),
        "file" => Some(NodeCategory::File),
        "module" => Some(NodeCategory::Module),
        "type" | "typedef" => Some(NodeCategory::Type),
        "function" | "fn" => Some(NodeCategory::Function),
        "variable" => Some(NodeCategory::Variable),
        "import" => Some(NodeCategory::Import),
        "constant" | "const" => Some(NodeCategory::Constant),
        "field" => Some(NodeCategory::Field),
        "parameter" | "param" => Some(NodeCategory::Parameter),
        "feature" => Some(NodeCategory::Feature),
        "component" => Some(NodeCategory::Component),
        "functional_centroid" | "centroid" => Some(NodeCategory::FunctionalCentroid),
        _ => None,
    }
}

fn parse_edge_type(s: &str) -> Option<EdgeType> {
    match s.to_lowercase().as_str() {
        "contains" => Some(EdgeType::Contains),
        "calls" => Some(EdgeType::Calls),
        "imports" => Some(EdgeType::Imports),
        "references" => Some(EdgeType::References),
        "implements" => Some(EdgeType::Implements),
        "extends" => Some(EdgeType::Extends),
        "depends_on" | "depends" => Some(EdgeType::DependsOn),
        "defines" => Some(EdgeType::Defines),
        "uses" => Some(EdgeType::Uses),
        "uses_type" => Some(EdgeType::UsesType),
        "ffi" | "ffi_binding" => Some(EdgeType::FfiBinding),
        "implements_feature" => Some(EdgeType::ImplementsFeature),
        "belongs_to_feature" => Some(EdgeType::BelongsToFeature),
        "contains_feature" => Some(EdgeType::ContainsFeature),
        "belongs_to_component" => Some(EdgeType::BelongsToComponent),
        _ => None,
    }
}

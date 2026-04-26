use std::collections::HashSet;
use std::sync::Arc;

use rmcp::{
    handler::server::router::tool::ToolRouter, model::*, tool, tool_handler, tool_router,
    ErrorData as McpError, ServerHandler,
};
use rpg_encoder::encoder::ValidationReport;
use rpg_encoder::{EdgeType, NodeCategory, RpgEncoder, RpgSnapshot, RpgStore};
use serde_json::{json, Map, Value};

use crate::state::{compute_dir_hash, save_dir_hash, AppState};

type JsonObject = Map<String, Value>;

fn get_str<'a>(params: &'a JsonObject, key: &str) -> Option<&'a str> {
    params.get(key).and_then(|v| v.as_str())
}

fn get_u64(params: &JsonObject, key: &str) -> Option<u64> {
    params.get(key).and_then(|v| v.as_u64())
}

#[derive(Clone)]
pub struct RpgService {
    state: Arc<AppState>,
    tool_router: ToolRouter<RpgService>,
}

#[tool_router]
impl RpgService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Full re-encode of the workspace repository")]
    async fn encode_repo(&self, _params: JsonObject) -> Result<CallToolResult, McpError> {
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

        let mut store_guard = self.state.store.write().expect("store lock poisoned");
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

    #[tool(description = "Get graph summary with validation report and language list")]
    async fn get_graph_summary(&self, _params: JsonObject) -> Result<CallToolResult, McpError> {
        let graph = self.state.graph.read().expect("graph lock poisoned");
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
    async fn search_nodes(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let query = get_str(&params, "query")
            .ok_or_else(|| McpError::invalid_params("missing 'query'", None))?;
        let kind = get_str(&params, "kind");
        let category = get_str(&params, "category");
        let limit = get_u64(&params, "limit").unwrap_or(50) as usize;

        let cat_filter: Option<NodeCategory> = category.and_then(parse_category);
        let query_lower = query.to_lowercase();

        let graph = self.state.graph.read().expect("graph lock poisoned");
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
                "category": format!("{:?}", node.category),
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
        description = "Get full details for a node by index, including incoming and outgoing edges"
    )]
    async fn get_node_details(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let id = get_u64(&params, "id")
            .ok_or_else(|| McpError::invalid_params("missing numeric 'id'", None))?
            as usize;

        let graph = self.state.graph.read().expect("graph lock poisoned");
        let node_id = rpg_encoder::NodeId::new(id);
        let node = graph
            .get_node(node_id)
            .ok_or_else(|| McpError::invalid_params(format!("node {id} not found"), None))?;

        let mut incoming: Vec<Value> = Vec::new();
        let mut outgoing: Vec<Value> = Vec::new();

        for (src, tgt, edge) in graph.edges() {
            if src == node_id {
                outgoing.push(json!({
                    "type": format!("{:?}", edge.edge_type),
                    "target": tgt.index(),
                }));
            }
            if tgt == node_id {
                incoming.push(json!({
                    "type": format!("{:?}", edge.edge_type),
                    "source": src.index(),
                }));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "id": node.id.index(),
                "name": node.name,
                "kind": node.kind,
                "category": format!("{:?}", node.category),
                "language": node.language,
                "path": node.path,
                "signature": node.signature,
                "description": node.description,
                "features": node.features,
                "feature_path": node.feature_path,
                "semantic_feature": node.semantic_feature,
                "documentation": node.documentation,
                "incoming_edges": incoming,
                "outgoing_edges": outgoing,
            })
            .to_string(),
        )]))
    }

    #[tool(description = "Get edges filtered by source/target/type, with limit")]
    async fn get_edges(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let source = get_u64(&params, "source").map(|v| v as usize);
        let target = get_u64(&params, "target").map(|v| v as usize);
        let edge_type = get_str(&params, "edge_type");
        let limit = get_u64(&params, "limit").unwrap_or(100) as usize;

        let type_filter: Option<EdgeType> = edge_type.and_then(parse_edge_type);

        let graph = self.state.graph.read().expect("graph lock poisoned");
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
                "type": format!("{:?}", edge.edge_type),
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
        description = "Get the skeleton: file nodes with their direct children (Contains edges)"
    )]
    async fn get_skeleton(&self, _params: JsonObject) -> Result<CallToolResult, McpError> {
        let graph = self.state.graph.read().expect("graph lock poisoned");

        let file_nodes: Vec<_> = graph
            .nodes()
            .filter(|n| n.category == NodeCategory::File)
            .collect();

        let mut skeleton: Vec<Value> = Vec::new();

        for file_node in &file_nodes {
            let file_id = file_node.id;
            let mut children: Vec<Value> = Vec::new();

            for (src, tgt, edge) in graph.edges() {
                if src == file_id && edge.edge_type == EdgeType::Contains {
                    if let Some(child) = graph.get_node(tgt) {
                        children.push(json!({
                            "id": child.id.index(),
                            "name": child.name,
                            "kind": child.kind,
                            "category": format!("{:?}", child.category),
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

        Ok(CallToolResult::success(vec![Content::text(
            json!({
                "files": skeleton,
                "total_files": skeleton.len(),
            })
            .to_string(),
        )]))
    }

    #[tool(
        description = "Get nodes with features or descriptions, optionally filtered by file path"
    )]
    async fn get_features(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let file_path = get_str(&params, "file_path");
        let limit = get_u64(&params, "limit").unwrap_or(100) as usize;

        let graph = self.state.graph.read().expect("graph lock poisoned");
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
                "category": format!("{:?}", node.category),
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
    async fn get_components(&self, params: JsonObject) -> Result<CallToolResult, McpError> {
        let limit = get_u64(&params, "limit").unwrap_or(100) as usize;

        let graph = self.state.graph.read().expect("graph lock poisoned");
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
                "RPG MCP server for code graph analysis. Use encode_repo to initialize, then query the graph."
                    .to_string(),
            ),
        }
    }
}

fn parse_category(s: &str) -> Option<NodeCategory> {
    match s.to_lowercase().as_str() {
        "repository" => Some(NodeCategory::Repository),
        "directory" => Some(NodeCategory::Directory),
        "file" => Some(NodeCategory::File),
        "module" => Some(NodeCategory::Module),
        "type" | "typedef" => Some(NodeCategory::Type),
        "function" | "fn" => Some(NodeCategory::Function),
        "import" => Some(NodeCategory::Import),
        "component" => Some(NodeCategory::Component),
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
        "ffi" | "ffi_binding" => Some(EdgeType::FfiBinding),
        _ => None,
    }
}

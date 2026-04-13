use crate::core::{Edge, EdgeType, Node, NodeCategory, NodeId, RpgGraph, SourceLocation};
use crate::error::Result;
use crate::parser::ParseResult;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct GraphBuilder {
    graph: RpgGraph,
    repo_name: Option<String>,
    repo_path: Option<PathBuf>,
    file_nodes: HashMap<PathBuf, NodeId>,
    dir_nodes: HashMap<PathBuf, NodeId>,
    unresolved_calls: Vec<(NodeId, crate::parser::CallInfo)>,
    unresolved_type_refs: Vec<(NodeId, crate::parser::TypeRefInfo)>,
    ffi_bindings: Vec<(NodeId, crate::languages::ffi::FfiBinding)>,
}

impl Default for GraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphBuilder {
    #[must_use = "GraphBuilder must be used"]
    pub fn new() -> Self {
        Self {
            graph: RpgGraph::new(),
            repo_name: None,
            repo_path: None,
            file_nodes: HashMap::new(),
            dir_nodes: HashMap::new(),
            unresolved_calls: Vec::new(),
            unresolved_type_refs: Vec::new(),
            ffi_bindings: Vec::new(),
        }
    }

    pub fn with_repo(mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        let name = name.into();
        let path = path.into();

        let node = Node::new(
            NodeId::new(self.graph.node_count()),
            NodeCategory::Repository,
            "repository",
            "unknown",
            &name,
        )
        .with_path(path.clone());

        self.graph.add_node(node);

        self.repo_name = Some(name);
        self.repo_path = Some(path);
        self
    }

    pub fn add_file(mut self, path: &Path, language: &str) -> Self {
        let path_buf = path.to_path_buf();
        if self.file_nodes.contains_key(&path_buf) {
            return self;
        }

        let parent_id = self.get_or_create_dir(path.parent().unwrap_or(Path::new("")), language);

        let node = Node::new(
            NodeId::new(self.graph.node_count()),
            NodeCategory::File,
            "file",
            language,
            path.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
        )
        .with_path(path_buf.clone());

        let id = self.graph.add_node(node);
        self.file_nodes.insert(path_buf, id);

        if let Some(parent_id) = parent_id {
            self.graph
                .add_edge(parent_id, id, Edge::new(EdgeType::Contains));
        }

        self
    }

    pub fn get_file_id(&self, path: &Path) -> Option<NodeId> {
        self.file_nodes.get(path).copied()
    }

    pub fn add_parsed_file(mut self, result: &ParseResult, language: &str) -> Self {
        if let Err(e) = self.add_parsed_file_internal(result, language) {
            tracing::warn!(
                "add_parsed_file failed for {}: {}",
                result.file_path.display(),
                e
            );
        }
        self
    }

    pub fn try_add_parsed_file(mut self, result: &ParseResult, language: &str) -> Result<Self> {
        self.add_parsed_file_internal(result, language)?;
        Ok(self)
    }

    fn add_parsed_file_internal(&mut self, result: &ParseResult, language: &str) -> Result<()> {
        let path_buf = result.file_path.clone();
        let file_id = if let Some(&id) = self.file_nodes.get(&path_buf) {
            id
        } else {
            let parent_id =
                self.get_or_create_dir(path_buf.parent().unwrap_or(Path::new("")), language);

            let node = Node::new(
                NodeId::new(self.graph.node_count()),
                NodeCategory::File,
                "file",
                language,
                path_buf
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("file"),
            )
            .with_path(path_buf.clone());

            let id = self.graph.add_node(node);
            self.file_nodes.insert(path_buf, id);

            if let Some(parent_id) = parent_id {
                self.graph
                    .add_edge(parent_id, id, Edge::new(EdgeType::Contains));
            }

            id
        };

        for def in &result.definitions {
            let category = match def.kind.as_str() {
                "fn" => NodeCategory::Function,
                "struct" | "enum" | "trait" | "type" | "class" | "interface" => NodeCategory::Type,
                "mod" | "module" => NodeCategory::Module,
                "const" | "constant" => NodeCategory::Constant,
                _ => NodeCategory::Variable,
            };

            let mut node = Node::new(
                NodeId::new(self.graph.node_count()),
                category,
                &def.kind,
                language,
                &def.name,
            )
            .with_path(result.file_path.clone())
            .with_signature(def.signature.clone().unwrap_or_default())
            .with_documentation(def.doc.clone().unwrap_or_default());

            if let Some(loc) = &def.location {
                node.location = Some(SourceLocation {
                    file: result.file_path.clone(),
                    start_line: loc.start_line,
                    start_column: loc.start_column,
                    end_line: loc.end_line,
                    end_column: loc.end_column,
                });
            }

            let node_id = self.graph.add_node(node);
            self.graph
                .add_edge(file_id, node_id, Edge::new(EdgeType::Contains));
        }

        for import in &result.imports {
            let node = Node::new(
                NodeId::new(self.graph.node_count()),
                NodeCategory::Import,
                "import",
                language,
                &import.module_path,
            )
            .with_path(result.file_path.clone());

            let node_id = self.graph.add_node(node);
            self.graph
                .add_edge(file_id, node_id, Edge::new(EdgeType::Contains));
        }

        for call in &result.calls {
            self.unresolved_calls.push((file_id, call.clone()));
        }

        for type_ref in &result.type_refs {
            self.unresolved_type_refs.push((file_id, type_ref.clone()));
        }

        for ffi in &result.ffi_bindings {
            self.ffi_bindings.push((file_id, ffi.clone()));
        }

        Ok(())
    }

    pub fn link_imports(mut self) -> Self {
        let definitions: HashMap<String, NodeId> = self
            .graph
            .nodes()
            .filter(|n| {
                matches!(
                    n.category,
                    NodeCategory::Type | NodeCategory::Function | NodeCategory::Module
                )
            })
            .map(|n| (n.name.clone(), n.id))
            .collect();

        let edges_to_add: Vec<(NodeId, NodeId)> = self
            .graph
            .nodes()
            .filter(|n| n.category == NodeCategory::Import)
            .filter_map(|node| {
                let parts: Vec<&str> = node.name.split("::").collect();
                if let Some(last) = parts.last() {
                    for lang in &["rust", "python", "go", "c#"] {
                        let key = format!("{}:{}", lang, last);
                        if let Some(&def_id) = definitions.get(&key) {
                            return Some((node.id, def_id));
                        }
                    }
                }
                None
            })
            .collect();

        for (src, tgt) in edges_to_add {
            self.graph
                .add_edge(src, tgt, Edge::new(EdgeType::References));
        }

        self
    }

    pub fn link_calls(mut self) -> Self {
        let definitions: HashMap<String, NodeId> = self
            .graph
            .nodes()
            .filter(|n| n.category == NodeCategory::Function)
            .map(|n| (n.name.clone(), n.id))
            .collect();

        let unresolved = std::mem::take(&mut self.unresolved_calls);

        for (caller_id, call) in unresolved {
            if let Some(&callee_id) = definitions.get(&call.callee) {
                let mut edge = Edge::new(EdgeType::Calls);
                if let Some(ref receiver) = call.receiver {
                    edge.metadata.insert(
                        "receiver".to_string(),
                        serde_json::Value::String(receiver.clone()),
                    );
                }
                edge.metadata.insert(
                    "call_kind".to_string(),
                    serde_json::Value::String(format!("{:?}", call.call_kind).to_lowercase()),
                );
                self.graph.add_edge(caller_id, callee_id, edge);
            }
        }

        self
    }

    pub fn link_type_refs(mut self) -> Self {
        let types: HashMap<String, NodeId> = self
            .graph
            .nodes()
            .filter(|n| n.category == NodeCategory::Type)
            .map(|n| (n.name.clone(), n.id))
            .collect();

        let unresolved = std::mem::take(&mut self.unresolved_type_refs);

        for (source_id, type_ref) in unresolved {
            if let Some(&type_id) = types.get(&type_ref.type_name) {
                let mut edge = Edge::new(EdgeType::UsesType);
                edge.metadata.insert(
                    "ref_kind".to_string(),
                    serde_json::Value::String(format!("{:?}", type_ref.ref_kind).to_lowercase()),
                );
                self.graph.add_edge(source_id, type_id, edge);
            }
        }

        self
    }

    pub fn link_all(self) -> Self {
        self.link_imports().link_calls().link_type_refs()
    }

    pub fn build(self) -> RpgGraph {
        self.graph
    }

    fn get_or_create_dir(&mut self, path: &Path, language: &str) -> Option<NodeId> {
        if path.as_os_str().is_empty() {
            return None;
        }

        let path_buf = path.to_path_buf();
        if let Some(&id) = self.dir_nodes.get(&path_buf) {
            return Some(id);
        }

        let node = Node::new(
            NodeId::new(self.graph.node_count()),
            NodeCategory::Directory,
            "directory",
            language,
            path.file_name().and_then(|n| n.to_str()).unwrap_or("dir"),
        )
        .with_path(path_buf.clone());

        let id = self.graph.add_node(node);
        self.dir_nodes.insert(path_buf, id);

        Some(id)
    }
}

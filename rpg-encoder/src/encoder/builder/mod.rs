use crate::core::{Edge, EdgeType, Node, NodeCategory, NodeId, RpgGraph, SourceLocation};
use crate::error::Result;
use crate::parser::{ImportInfo, ParseResult};
use rustc_hash::FxHashMap;
use std::path::{Path, PathBuf};

mod calls;
mod extends;
mod ffi;
mod impls;
mod imports;
mod type_refs;

pub struct GraphBuilder {
    pub(crate) graph: RpgGraph,
    repo_name: Option<String>,
    repo_path: Option<PathBuf>,
    pub(crate) file_nodes: FxHashMap<PathBuf, NodeId>,
    dir_nodes: FxHashMap<PathBuf, NodeId>,
    pub(crate) unresolved_calls: Vec<(NodeId, crate::parser::CallInfo, PathBuf)>,
    pub(crate) unresolved_type_refs: Vec<(NodeId, crate::parser::TypeRefInfo, PathBuf)>,
    pub(crate) unresolved_impls: Vec<(NodeId, String)>,
    pub(crate) ffi_bindings: Vec<(NodeId, crate::languages::ffi::FfiBinding)>,
    pub(crate) file_imports: FxHashMap<PathBuf, Vec<ImportInfo>>,
    pub(crate) qualified_defs: FxHashMap<(PathBuf, String), NodeId>,
    pub(crate) bare_name_defs: FxHashMap<String, Vec<(PathBuf, NodeId)>>,
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
            file_nodes: FxHashMap::default(),
            dir_nodes: FxHashMap::default(),
            unresolved_calls: Vec::new(),
            unresolved_type_refs: Vec::new(),
            unresolved_impls: Vec::new(),
            ffi_bindings: Vec::new(),
            file_imports: FxHashMap::default(),
            qualified_defs: FxHashMap::default(),
            bare_name_defs: FxHashMap::default(),
        }
    }

    /// Relativize an absolute path against the repo root.
    ///
    /// All node paths, location files, and lookup keys are stored repo-relative
    /// so that `.rpg/base.json` is portable across machines (the committed-graph
    /// workflow). If `repo_path` is unset or the path doesn't start with it,
    /// the path is returned unchanged.
    fn relativize(&self, path: &Path) -> PathBuf {
        match &self.repo_path {
            Some(root) => path.strip_prefix(root).map(PathBuf::from).unwrap_or_else(|_| path.to_path_buf()),
            None => path.to_path_buf(),
        }
    }

    pub fn with_repo(mut self, name: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        let name = name.into();
        let path = path.into();

        // The Repository node's path is "." (it IS the root). This keeps it
        // relative rather than embedding the absolute repo path in the graph.
        let node = Node::new(
            NodeId::new(self.graph.node_count()),
            NodeCategory::Repository,
            "repository",
            "unknown",
            &name,
        )
        .with_path(PathBuf::from("."));

        self.graph.add_node(node);

        self.repo_name = Some(name);
        self.repo_path = Some(path);
        self
    }

    pub fn add_file(mut self, path: &Path, language: &str) -> Self {
        let path_buf = self.relativize(path);
        if self.file_nodes.contains_key(&path_buf) {
            return self;
        }

        let parent_id = self.get_or_create_dir(path_buf.parent().unwrap_or(Path::new("")), language);

        let node = Node::new(
            NodeId::new(self.graph.node_count()),
            NodeCategory::File,
            "file",
            language,
            path_buf.file_name().and_then(|n| n.to_str()).unwrap_or("file"),
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
        // Relativize the file path against the repo root so all stored paths
        // are repo-relative (portable across machines for the committed-graph
        // workflow).
        let path_buf = self.relativize(&result.file_path);
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
            self.file_nodes.insert(path_buf.clone(), id);

            if let Some(parent_id) = parent_id {
                self.graph
                    .add_edge(parent_id, id, Edge::new(EdgeType::Contains));
            }

            id
        };

        self.file_imports
            .insert(path_buf.clone(), result.imports.clone());

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
            .with_path(path_buf.clone())
            .with_signature(def.signature.clone().unwrap_or_default())
            .with_documentation(def.doc.clone().unwrap_or_default());

            node.metadata = def.metadata.clone();

            if let Some(loc) = &def.location {
                node.location = Some(SourceLocation {
                    file: path_buf.clone(),
                    start_line: loc.start_line,
                    start_column: loc.start_column,
                    end_line: loc.end_line,
                    end_column: loc.end_column,
                });
            }

            let node_id = self.graph.add_node(node);
            self.graph
                .add_edge(file_id, node_id, Edge::new(EdgeType::Contains));

            self.qualified_defs
                .insert((path_buf.clone(), def.name.clone()), node_id);
            self.bare_name_defs
                .entry(def.name.clone())
                .or_default()
                .push((path_buf.clone(), node_id));

            if def.kind == "impl_trait" {
                if let Some(trait_val) = def.metadata.get("trait") {
                    if let Some(trait_name) = trait_val.as_str() {
                        self.unresolved_impls
                            .push((node_id, trait_name.to_string()));
                    }
                }
            }
        }

        for import in &result.imports {
            let node = Node::new(
                NodeId::new(self.graph.node_count()),
                NodeCategory::Import,
                "import",
                language,
                &import.module_path,
            )
            .with_path(path_buf.clone());

            let node_id = self.graph.add_node(node);
            self.graph
                .add_edge(file_id, node_id, Edge::new(EdgeType::Contains));
        }

        for call in &result.calls {
            self.unresolved_calls
                .push((file_id, call.clone(), path_buf.clone()));
        }

        for type_ref in &result.type_refs {
            self.unresolved_type_refs
                .push((file_id, type_ref.clone(), path_buf.clone()));
        }

        for ffi in &result.ffi_bindings {
            self.ffi_bindings.push((file_id, ffi.clone()));
        }

        Ok(())
    }

    pub fn link_all(self) -> Self {
        self.link_imports()
            .link_calls()
            .link_type_refs()
            .link_impls()
            .link_extends()
            .link_ffi()
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

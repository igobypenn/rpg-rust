use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::edge::{Edge, EdgeType};
use super::id::{EdgeId, NodeId};
use super::node::{Node, NodeCategory, NodeLevel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpgGraph {
    #[serde(
        serialize_with = "serialize_graph",
        deserialize_with = "deserialize_graph"
    )]
    graph: DiGraph<Node, Edge>,
    #[serde(skip)]
    node_id_map: HashMap<NodeId, NodeIndex>,
    #[serde(skip)]
    edge_id_map: HashMap<EdgeId, EdgeIndex>,
    #[serde(skip)]
    next_node_id: usize,
    #[serde(skip)]
    next_edge_id: usize,
}

fn serialize_graph<S>(graph: &DiGraph<Node, Edge>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let nodes: Vec<&Node> = graph.node_indices().map(|idx| &graph[idx]).collect();
    let edges: Vec<(NodeId, NodeId, &Edge)> = graph
        .edge_indices()
        .filter_map(|eidx| {
            let (source, target) = graph.edge_endpoints(eidx)?;
            let source_id = graph[source].id;
            let target_id = graph[target].id;
            Some((source_id, target_id, &graph[eidx]))
        })
        .collect();

    #[derive(Serialize)]
    struct GraphData<'a> {
        nodes: Vec<&'a Node>,
        edges: Vec<(NodeId, NodeId, &'a Edge)>,
    }

    GraphData { nodes, edges }.serialize(serializer)
}

fn deserialize_graph<'de, D>(deserializer: D) -> Result<DiGraph<Node, Edge>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct GraphData {
        nodes: Vec<Node>,
        edges: Vec<(NodeId, NodeId, Edge)>,
    }

    let data = GraphData::deserialize(deserializer)?;
    let mut graph = DiGraph::new();

    let mut node_id_to_index = HashMap::new();
    for mut node in data.nodes {
        let idx = graph.add_node(Node {
            id: node.id,
            category: node.category,
            kind: std::mem::take(&mut node.kind),
            language: std::mem::take(&mut node.language),
            name: std::mem::take(&mut node.name),
            path: node.path.take(),
            location: node.location.take(),
            metadata: std::mem::take(&mut node.metadata),
            description: node.description.take(),
            features: std::mem::take(&mut node.features),
            feature_path: node.feature_path.take(),
            signature: node.signature.take(),
            documentation: node.documentation.take(),
            source_ref: node.source_ref.take(),
            semantic_feature: node.semantic_feature.take(),
            node_level: node.node_level,
        });
        node_id_to_index.insert(node.id, idx);
    }

    for (source_id, target_id, edge) in data.edges {
        if let (Some(&source_idx), Some(&target_idx)) = (
            node_id_to_index.get(&source_id),
            node_id_to_index.get(&target_id),
        ) {
            graph.add_edge(source_idx, target_idx, edge);
        }
    }

    Ok(graph)
}

impl Default for RpgGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl RpgGraph {
    #[must_use = "RpgGraph must be used"]
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_id_map: HashMap::new(),
            edge_id_map: HashMap::new(),
            next_node_id: 0,
            next_edge_id: 0,
        }
    }

    pub fn add_node(&mut self, mut node: Node) -> NodeId {
        let id = NodeId::new(self.next_node_id);
        node.id = id;
        self.next_node_id += 1;

        let idx = self.graph.add_node(node);
        self.node_id_map.insert(id, idx);

        id
    }

    pub fn add_edge(&mut self, source: NodeId, target: NodeId, edge: Edge) -> EdgeId {
        let id = EdgeId::new(self.next_edge_id);
        self.next_edge_id += 1;

        if let (Some(&sidx), Some(&tidx)) =
            (self.node_id_map.get(&source), self.node_id_map.get(&target))
        {
            let eidx = self.graph.add_edge(sidx, tidx, edge);
            self.edge_id_map.insert(id, eidx);
        }

        id
    }

    pub fn add_typed_edge(
        &mut self,
        source: NodeId,
        target: NodeId,
        edge_type: EdgeType,
    ) -> EdgeId {
        self.add_edge(source, target, Edge::new(edge_type))
    }

    pub fn get_node(&self, id: NodeId) -> Option<&Node> {
        self.node_id_map
            .get(&id)
            .and_then(|&idx| self.graph.node_weight(idx))
    }

    pub fn get_node_mut(&mut self, id: NodeId) -> Option<&mut Node> {
        self.node_id_map
            .get(&id)
            .and_then(|&idx| self.graph.node_weight_mut(idx))
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.graph
            .node_indices()
            .filter_map(move |idx| self.graph.node_weight(idx))
    }

    pub fn edges(&self) -> impl Iterator<Item = (NodeId, NodeId, &Edge)> {
        self.graph.edge_indices().filter_map(move |eidx| {
            let (source, target) = self.graph.edge_endpoints(eidx)?;
            let source_id = self.graph.node_weight(source)?.id;
            let target_id = self.graph.node_weight(target)?.id;
            let edge = self.graph.edge_weight(eidx)?;
            Some((source_id, target_id, edge))
        })
    }

    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    pub fn find_node_by_path(&self, path: &Path) -> Option<&Node> {
        self.nodes()
            .find(|n| n.path.as_ref().map(|p| p == path).unwrap_or(false))
    }

    pub fn find_node_by_name(&self, name: &str, category: Option<NodeCategory>) -> Option<&Node> {
        self.nodes()
            .find(|n| n.name == name && category.is_none_or(|c| n.category == c))
    }

    /// Returns an iterator over low-level (V^L) nodes.
    ///
    /// V^L nodes are implementation-level entities (functions, types, etc.)
    /// as opposed to high-level functional centroids (V^H).
    pub fn low_level_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes().filter(|n| n.node_level == NodeLevel::Low)
    }

    /// Returns an iterator over functional centroid (V^H) nodes.
    ///
    /// V^H nodes are high-level functional abstractions created
    /// during the functional abstraction phase.
    pub fn functional_centroids(&self) -> impl Iterator<Item = &Node> {
        self.nodes().filter(|n| n.node_level == NodeLevel::High)
    }

    /// Ground a functional centroid by aggregating metadata from its V^L children.
    ///
    /// Per the paper: "populate the missing metadata m for nodes in V^H through
    /// bottom-up propagation, utilizing a Lowest Common Ancestor (LCA) mechanism."
    ///
    /// Returns the grounded centroid node if successful.
    pub fn ground_centroid(&mut self, centroid_id: NodeId) -> Option<&Node> {
        let &centroid_idx = self.node_id_map.get(&centroid_id)?;

        // Collect V^L nodes that belong to this centroid via BelongsToFeature edges
        let mut children_paths: Vec<PathBuf> = Vec::new();
        let mut children_features: Vec<String> = Vec::new();

        for edge_ref in self
            .graph
            .edges_directed(centroid_idx, petgraph::Direction::Incoming)
        {
            let edge = edge_ref.weight();
            if edge.edge_type == EdgeType::BelongsToFeature {
                let source_idx = edge_ref.source();
                if let Some(child) = self.graph.node_weight(source_idx) {
                    if let Some(p) = &child.path {
                        children_paths.push(p.clone());
                    }
                    if let Some(f) = &child.semantic_feature {
                        children_features.push(f.clone());
                    }
                }
            }
        }

        if children_paths.is_empty() && children_features.is_empty() {
            return None;
        }

        // Update the centroid with aggregated info
        if let Some(centroid) = self.get_node_mut(centroid_id) {
            // Derive path from common ancestor of children
            if centroid.path.is_none() && !children_paths.is_empty() {
                // Find common directory prefix
                let common_path = find_common_ancestor(&children_paths);
                centroid.path = Some(common_path);
            }

            // Aggregate semantic features
            if !children_features.is_empty() {
                centroid.semantic_feature = Some(children_features.join("; "));
            }

            return self.get_node(centroid_id);
        }

        None
    }

    pub fn children_of(&self, parent_id: NodeId) -> Vec<&Node> {
        let Some(&parent_idx) = self.node_id_map.get(&parent_id) else {
            return Vec::new();
        };

        self.graph
            .edges(parent_idx)
            .filter_map(|edge_ref| {
                let edge = edge_ref.weight();
                if edge.edge_type == EdgeType::Contains {
                    let target_idx = edge_ref.target();
                    self.graph.node_weight(target_idx)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn to_petgraph(&self) -> DiGraph<Node, Edge> {
        self.graph.clone()
    }

    pub fn remove_node(&mut self, id: NodeId) -> Option<Node> {
        let idx = self.node_id_map.remove(&id)?;
        let node = self.graph.remove_node(idx)?;
        self.edge_id_map
            .retain(|_, &mut eidx| self.graph.edge_endpoints(eidx).is_some());
        Some(node)
    }

    pub fn remove_file_nodes(&mut self, file_path: &Path) -> Vec<NodeId> {
        let removed: Vec<NodeId> = self
            .nodes()
            .filter(|n| n.path.as_ref().map(|p| p == file_path).unwrap_or(false))
            .map(|n| n.id)
            .collect();

        for &id in &removed {
            self.remove_node(id);
        }

        removed
    }

    pub fn nodes_for_file(&self, file_path: &Path) -> Vec<&Node> {
        self.nodes()
            .filter(|n| n.path.as_ref().map(|p| p == file_path).unwrap_or(false))
            .collect()
    }

    pub fn update_node_semantics(
        &mut self,
        id: NodeId,
        features: Vec<String>,
        description: String,
        feature_path: String,
    ) -> bool {
        if let Some(node) = self.get_node_mut(id) {
            node.features = features;
            node.description = Some(description);
            node.feature_path = Some(feature_path);
            true
        } else {
            false
        }
    }

    pub fn find_node_by_location(&self, file_path: &Path, line: usize) -> Option<&Node> {
        self.nodes().find(|n| {
            n.path.as_ref().map(|p| p == file_path).unwrap_or(false)
                && n.location
                    .as_ref()
                    .map(|l| l.start_line == line)
                    .unwrap_or(false)
        })
    }

    pub fn find_node_in_file(&self, file_path: &Path, name: &str) -> Option<&Node> {
        self.nodes()
            .find(|n| n.path.as_ref().map(|p| p == file_path).unwrap_or(false) && n.name == name)
    }

    pub fn remove_edges_for_nodes(&mut self, node_ids: &[NodeId]) -> usize {
        let node_set: std::collections::HashSet<NodeId> = node_ids.iter().copied().collect();
        let original_count = self.graph.edge_count();

        let edges_to_remove: Vec<_> = self
            .graph
            .edge_indices()
            .filter(|&eidx| {
                if let Some((source, target)) = self.graph.edge_endpoints(eidx) {
                    let source_id = self.graph.node_weight(source).map(|n| n.id);
                    let target_id = self.graph.node_weight(target).map(|n| n.id);
                    match (source_id, target_id) {
                        (Some(s), Some(t)) => node_set.contains(&s) || node_set.contains(&t),
                        _ => false,
                    }
                } else {
                    false
                }
            })
            .collect();

        for eidx in edges_to_remove {
            self.graph.remove_edge(eidx);
        }

        self.edge_id_map.clear();
        for eidx in self.graph.edge_indices() {
            self.edge_id_map.insert(EdgeId::new(eidx.index()), eidx);
        }

        original_count - self.graph.edge_count()
    }

    pub fn edges_involving(&self, node_id: NodeId) -> Vec<(EdgeId, NodeId, NodeId, EdgeType)> {
        let Some(&idx) = self.node_id_map.get(&node_id) else {
            return Vec::new();
        };

        let mut results = Vec::new();

        for eidx in self.graph.edge_indices() {
            if let Some((source, target)) = self.graph.edge_endpoints(eidx) {
                if source == idx || target == idx {
                    if let (Some(source_node), Some(target_node), Some(edge)) = (
                        self.graph.node_weight(source),
                        self.graph.node_weight(target),
                        self.graph.edge_weight(eidx),
                    ) {
                        results.push((
                            EdgeId::new(eidx.index()),
                            source_node.id,
                            target_node.id,
                            edge.edge_type,
                        ));
                    }
                }
            }
        }

        results
    }

    pub fn node_exists(&self, id: NodeId) -> bool {
        self.node_id_map
            .get(&id)
            .and_then(|&idx| self.graph.node_weight(idx))
            .map(|n| !n.name.is_empty())
            .unwrap_or(false)
    }

    pub fn nodes_mut(&mut self) -> impl Iterator<Item = &mut Node> {
        self.graph.node_weights_mut()
    }

    pub fn retain_edges<F>(&mut self, mut f: F)
    where
        F: FnMut(NodeId, NodeId, &Edge) -> bool,
    {
        let edges_to_remove: Vec<_> = self
            .graph
            .edge_indices()
            .filter(|&eidx| {
                if let Some((source, target)) = self.graph.edge_endpoints(eidx) {
                    if let (Some(s_node), Some(t_node), Some(edge)) = (
                        self.graph.node_weight(source),
                        self.graph.node_weight(target),
                        self.graph.edge_weight(eidx),
                    ) {
                        return !f(s_node.id, t_node.id, edge);
                    }
                }
                true
            })
            .collect();

        for eidx in edges_to_remove {
            self.graph.remove_edge(eidx);
        }

        self.edge_id_map.clear();
        for eidx in self.graph.edge_indices() {
            self.edge_id_map.insert(EdgeId::new(eidx.index()), eidx);
        }
    }

    pub fn neighbors(&self, id: NodeId) -> Vec<&Node> {
        let Some(&idx) = self.node_id_map.get(&id) else {
            return Vec::new();
        };

        self.graph
            .neighbors(idx)
            .filter_map(|neighbor_idx| self.graph.node_weight(neighbor_idx))
            .collect()
    }

    pub fn predecessors(&self, id: NodeId) -> Vec<&Node> {
        let Some(&idx) = self.node_id_map.get(&id) else {
            return Vec::new();
        };

        self.graph
            .neighbors_directed(idx, petgraph::Direction::Incoming)
            .filter_map(|neighbor_idx| self.graph.node_weight(neighbor_idx))
            .collect()
    }

    pub fn successors(&self, id: NodeId) -> Vec<&Node> {
        let Some(&idx) = self.node_id_map.get(&id) else {
            return Vec::new();
        };

        self.graph
            .neighbors_directed(idx, petgraph::Direction::Outgoing)
            .filter_map(|neighbor_idx| self.graph.node_weight(neighbor_idx))
            .collect()
    }

    pub fn edge_between(&self, source: NodeId, target: NodeId) -> Option<&Edge> {
        let (&sidx, &tidx) = (
            self.node_id_map.get(&source)?,
            self.node_id_map.get(&target)?,
        );

        self.graph
            .edges_connecting(sidx, tidx)
            .next()
            .map(|e| e.weight())
    }

    /// Remove the edge between two nodes, if any.
    ///
    /// Returns true if an edge was removed, false otherwise.
    pub fn remove_edge_between(&mut self, source: NodeId, target: NodeId) -> bool {
        let (sidx, tidx) = match (
            self.node_id_map.get(&source).copied(),
            self.node_id_map.get(&target).copied(),
        ) {
            (Some(s), Some(t)) => (s, t),
            _ => return false,
        };

        // Find the edge index
        let eidx = match self.graph.edges_connecting(sidx, tidx).next() {
            Some(e) => e.id(),
            None => return false,
        };

        // Remove the edge
        self.graph.remove_edge(eidx);

        // Rebuild edge_id_map since indices may have changed
        self.edge_id_map.clear();
        for eidx in self.graph.edge_indices() {
            self.edge_id_map.insert(EdgeId::new(eidx.index()), eidx);
        }

        true
    }

    pub fn edges_from(&self, source: NodeId) -> Vec<(NodeId, &Edge)> {
        let Some(&idx) = self.node_id_map.get(&source) else {
            return Vec::new();
        };

        self.graph
            .edges(idx)
            .filter_map(|edge_ref| {
                let target_idx = edge_ref.target();
                let target_node = self.graph.node_weight(target_idx)?;
                Some((target_node.id, edge_ref.weight()))
            })
            .collect()
    }

    pub fn edges_to(&self, target: NodeId) -> Vec<(NodeId, &Edge)> {
        let Some(&idx) = self.node_id_map.get(&target) else {
            return Vec::new();
        };

        self.graph
            .edges_directed(idx, petgraph::Direction::Incoming)
            .filter_map(|edge_ref| {
                let source_idx = edge_ref.source();
                let source_node = self.graph.node_weight(source_idx)?;
                Some((source_node.id, edge_ref.weight()))
            })
            .collect()
    }

    pub fn as_petgraph(&self) -> &DiGraph<Node, Edge> {
        &self.graph
    }

    pub fn into_petgraph(self) -> DiGraph<Node, Edge> {
        self.graph
    }
}

fn find_common_ancestor(paths: &[PathBuf]) -> PathBuf {
    if paths.is_empty() {
        return PathBuf::new();
    }

    if paths.len() == 1 {
        return paths[0]
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();
    }

    // Start with the first path's components
    let mut common: Vec<&std::ffi::OsStr> = paths[0].iter().collect();

    // Intersect with each subsequent path
    for path in &paths[1..] {
        let components: Vec<&std::ffi::OsStr> = path.iter().collect();
        let new_len = common.len().min(components.len());
        common.truncate(new_len);

        for i in 0..new_len {
            if common[i] != components[i] {
                common.truncate(i);
                break;
            }
        }
    }

    // If we have components, return as path; otherwise return empty
    if common.is_empty() {
        PathBuf::new()
    } else {
        common.iter().fold(PathBuf::new(), |mut acc, &comp| {
            acc.push(comp);
            acc
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_graph() -> RpgGraph {
        let mut graph = RpgGraph::new();

        let repo = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Repository,
            "repository",
            "rust",
            "test-repo",
        ));

        let file = graph.add_node(Node::new(
            NodeId::new(1),
            NodeCategory::File,
            "file",
            "rust",
            "main.rs",
        ));

        let func = graph.add_node(Node::new(
            NodeId::new(2),
            NodeCategory::Function,
            "function",
            "rust",
            "main",
        ));

        graph.add_typed_edge(repo, file, EdgeType::Contains);
        graph.add_typed_edge(file, func, EdgeType::Contains);

        graph
    }

    #[test]
    fn test_new_graph_is_empty() {
        let graph = RpgGraph::new();
        assert_eq!(graph.node_count(), 0);
        assert_eq!(graph.edge_count(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut graph = RpgGraph::new();
        let id = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "test_func",
        ));

        assert_eq!(graph.node_count(), 1);
        assert!(graph.get_node(id).is_some());
        assert_eq!(graph.get_node(id).unwrap().name, "test_func");
    }

    #[test]
    fn test_add_edge() {
        let mut graph = RpgGraph::new();

        let n1 = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "func1",
        ));
        let n2 = graph.add_node(Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "function",
            "rust",
            "func2",
        ));

        let _eid = graph.add_typed_edge(n1, n2, EdgeType::Calls);

        assert_eq!(graph.edge_count(), 1);
        let edges: Vec<_> = graph.edges().collect();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].0, n1);
        assert_eq!(edges[0].1, n2);
        assert_eq!(edges[0].2.edge_type, EdgeType::Calls);
    }

    #[test]
    fn test_children_of() {
        let graph = create_test_graph();

        let repo_id = NodeId::new(0);
        let children = graph.children_of(repo_id);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].name, "main.rs");

        let file_id = NodeId::new(1);
        let file_children = graph.children_of(file_id);
        assert_eq!(file_children.len(), 1);
        assert_eq!(file_children[0].name, "main");
    }

    #[test]
    fn test_remove_node() {
        let mut graph = create_test_graph();
        let file_id = NodeId::new(1);

        let removed = graph.remove_node(file_id);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "main.rs");
        assert!(graph.get_node(file_id).is_none());
    }

    #[test]
    fn test_remove_file_nodes() {
        let mut graph = RpgGraph::new();

        graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::File,
                "file",
                "rust",
                "main.rs",
            )
            .with_path(std::path::PathBuf::from("src/main.rs")),
        );

        graph.add_node(
            Node::new(
                NodeId::new(1),
                NodeCategory::Function,
                "function",
                "rust",
                "foo",
            )
            .with_path(std::path::PathBuf::from("src/main.rs")),
        );

        graph.add_node(
            Node::new(NodeId::new(2), NodeCategory::File, "file", "rust", "lib.rs")
                .with_path(std::path::PathBuf::from("src/lib.rs")),
        );

        let removed = graph.remove_file_nodes(std::path::Path::new("src/main.rs"));
        assert_eq!(removed.len(), 2);
        assert_eq!(graph.node_count(), 1);
    }

    #[test]
    fn test_find_node_by_name() {
        let graph = create_test_graph();

        let found = graph.find_node_by_name("main", Some(NodeCategory::Function));
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "main");

        let not_found = graph.find_node_by_name("nonexistent", None);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_node_exists() {
        let graph = create_test_graph();

        assert!(graph.node_exists(NodeId::new(0)));
        assert!(graph.node_exists(NodeId::new(1)));
        assert!(!graph.node_exists(NodeId::new(99)));
    }

    #[test]
    fn test_edges_involving() {
        let graph = create_test_graph();

        let edges = graph.edges_involving(NodeId::new(0));
        assert_eq!(edges.len(), 1);

        let no_edges = graph.edges_involving(NodeId::new(2));
        assert_eq!(no_edges.len(), 1);
    }

    #[test]
    fn test_retain_edges() {
        let mut graph = RpgGraph::new();

        let n1 = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "func1",
        ));
        let n2 = graph.add_node(Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "function",
            "rust",
            "func2",
        ));
        let n3 = graph.add_node(Node::new(
            NodeId::new(2),
            NodeCategory::Function,
            "function",
            "rust",
            "func3",
        ));

        graph.add_typed_edge(n1, n2, EdgeType::Calls);
        graph.add_typed_edge(n2, n3, EdgeType::Calls);

        assert_eq!(graph.edge_count(), 2);

        graph.retain_edges(|_, _, e| e.edge_type == EdgeType::Calls);
        assert_eq!(graph.edge_count(), 2);

        graph.retain_edges(|source, _, _| source == n1);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_to_petgraph() {
        let graph = create_test_graph();
        let pg = graph.to_petgraph();

        assert_eq!(pg.node_count(), 3);
        assert_eq!(pg.edge_count(), 2);
    }

    #[test]
    fn test_neighbors() {
        let mut graph = RpgGraph::new();

        let n1 = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "func1",
        ));
        let n2 = graph.add_node(Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "function",
            "rust",
            "func2",
        ));
        let n3 = graph.add_node(Node::new(
            NodeId::new(2),
            NodeCategory::Function,
            "function",
            "rust",
            "func3",
        ));

        graph.add_typed_edge(n1, n2, EdgeType::Calls);
        graph.add_typed_edge(n1, n3, EdgeType::Calls);

        let neighbors = graph.neighbors(n1);
        assert_eq!(neighbors.len(), 2);
    }

    #[test]
    fn test_predecessors_and_successors() {
        let mut graph = RpgGraph::new();

        let n1 = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "func1",
        ));
        let n2 = graph.add_node(Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "function",
            "rust",
            "func2",
        ));

        graph.add_typed_edge(n1, n2, EdgeType::Calls);

        let successors = graph.successors(n1);
        assert_eq!(successors.len(), 1);
        assert_eq!(successors[0].name, "func2");

        let predecessors = graph.predecessors(n2);
        assert_eq!(predecessors.len(), 1);
        assert_eq!(predecessors[0].name, "func1");
    }

    #[test]
    fn test_edge_between() {
        let mut graph = RpgGraph::new();

        let n1 = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "func1",
        ));
        let n2 = graph.add_node(Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "function",
            "rust",
            "func2",
        ));

        graph.add_typed_edge(n1, n2, EdgeType::Calls);

        let edge = graph.edge_between(n1, n2);
        assert!(edge.is_some());
        assert_eq!(edge.unwrap().edge_type, EdgeType::Calls);

        let no_edge = graph.edge_between(n2, n1);
        assert!(no_edge.is_none());
    }

    #[test]
    fn test_edges_from_and_to() {
        let mut graph = RpgGraph::new();

        let n1 = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "func1",
        ));
        let n2 = graph.add_node(Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "function",
            "rust",
            "func2",
        ));

        graph.add_typed_edge(n1, n2, EdgeType::Calls);

        let from_n1 = graph.edges_from(n1);
        assert_eq!(from_n1.len(), 1);
        assert_eq!(from_n1[0].0, n2);

        let to_n2 = graph.edges_to(n2);
        assert_eq!(to_n2.len(), 1);
        assert_eq!(to_n2[0].0, n1);
    }

    #[test]
    fn test_serialization() {
        let graph = create_test_graph();
        let json = serde_json::to_string(&graph).unwrap();
        let deserialized: RpgGraph = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.node_count(), graph.node_count());
        assert_eq!(deserialized.edge_count(), graph.edge_count());
    }

    #[test]
    fn test_as_petgraph() {
        let graph = create_test_graph();
        let pg_ref = graph.as_petgraph();
        assert_eq!(pg_ref.node_count(), 3);
    }

    #[test]
    fn test_remove_edges_for_nodes() {
        let mut graph = RpgGraph::new();

        let n1 = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "func1",
        ));
        let n2 = graph.add_node(Node::new(
            NodeId::new(1),
            NodeCategory::Function,
            "function",
            "rust",
            "func2",
        ));
        let n3 = graph.add_node(Node::new(
            NodeId::new(2),
            NodeCategory::Function,
            "function",
            "rust",
            "func3",
        ));

        graph.add_typed_edge(n1, n2, EdgeType::Calls);
        graph.add_typed_edge(n2, n3, EdgeType::Calls);

        assert_eq!(graph.edge_count(), 2);

        let removed = graph.remove_edges_for_nodes(&[n1]);
        assert_eq!(removed, 1);
        assert_eq!(graph.edge_count(), 1);
    }

    #[test]
    fn test_update_node_semantics() {
        let mut graph = RpgGraph::new();

        let id = graph.add_node(Node::new(
            NodeId::new(0),
            NodeCategory::Function,
            "function",
            "rust",
            "test_func",
        ));

        let result = graph.update_node_semantics(
            id,
            vec!["feature1".to_string()],
            "description".to_string(),
            "path/to/feature".to_string(),
        );

        assert!(result);
        let node = graph.get_node(id).unwrap();
        assert_eq!(node.features, vec!["feature1"]);
        assert_eq!(node.description, Some("description".to_string()));
    }

    #[test]
    fn test_find_node_by_location() {
        let mut graph = RpgGraph::new();

        let loc = crate::core::location::SourceLocation::new(
            std::path::PathBuf::from("src/main.rs"),
            10,
            1,
            20,
            1,
        );

        graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::Function,
                "function",
                "rust",
                "test_func",
            )
            .with_path(std::path::PathBuf::from("src/main.rs"))
            .with_location(loc),
        );

        let found = graph.find_node_by_location(std::path::Path::new("src/main.rs"), 10);
        assert!(found.is_some());

        let not_found = graph.find_node_by_location(std::path::Path::new("src/main.rs"), 99);
        assert!(not_found.is_none());
    }
}

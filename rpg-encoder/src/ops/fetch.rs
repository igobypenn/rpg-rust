//! FetchNode Tool - Retrieve detailed node information.
//!
//! Per the paper: "Given a node identifier n, FetchNode(n) returns the
//! complete node details including all metadata, edges, and context."

use crate::core::{EdgeType, Node, NodeCategory, NodeId, RpgGraph};

/// Summary of a related node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSummary {
    pub id: NodeId,
    pub name: String,
    pub category: NodeCategory,
}

impl From<&Node> for NodeSummary {
    fn from(node: &Node) -> Self {
        NodeSummary {
            id: node.id,
            name: node.name.clone(),
            category: node.category,
        }
    }
}

impl From<Node> for NodeSummary {
    fn from(node: Node) -> Self {
        NodeSummary::from(&node)
    }
}

/// Direction of an edge relative to the focal node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EdgeDirection {
    Incoming,
    Outgoing,
}

/// Information about an edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeInfo {
    pub edge_type: EdgeType,
    pub other_node: NodeSummary,
    pub direction: EdgeDirection,
}

/// Detailed node information.
#[derive(Debug, Clone)]
pub struct NodeDetail {
    /// The node itself.
    pub node: Node,
    /// Incoming edges (dependencies on this node).
    pub incoming: Vec<EdgeInfo>,
    /// Outgoing edges (this node's dependencies).
    pub outgoing: Vec<EdgeInfo>,
    /// Parent nodes (containers).
    pub parents: Vec<NodeSummary>,
    /// Child nodes (contained).
    pub children: Vec<NodeSummary>,
    /// Functional centroid this node belongs to (if V^L).
    pub functional_centroid: Option<NodeSummary>,
    /// Members of this functional centroid (if V^H).
    pub centroid_members: Vec<NodeSummary>,
}

/// Result of a fetch operation.
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub detail: Option<NodeDetail>,
    pub error: Option<String>,
}

/// FetchNode tool for retrieving node details.
pub struct FetchNode<'a> {
    graph: &'a RpgGraph,
}

impl<'a> FetchNode<'a> {
    /// Create a new FetchNode tool.
    #[must_use = "FetchNode should be used to fetch node details"]
    pub fn new(graph: &'a RpgGraph) -> Self {
        Self { graph }
    }

    /// Fetch detailed information about a node.
    pub fn fetch(&self, node_id: NodeId) -> FetchResult {
        let Some(node) = self.graph.get_node(node_id) else {
            return FetchResult {
                detail: None,
                error: Some(format!("Node {:?} not found", node_id)),
            };
        };

        let detail = NodeDetail {
            node: node.clone(),
            incoming: self.get_incoming_edges(node_id),
            outgoing: self.get_outgoing_edges(node_id),
            parents: self.get_parents(node_id),
            children: self.get_children(node_id),
            functional_centroid: self.get_functional_centroid(node_id),
            centroid_members: self.get_centroid_members(node_id),
        };

        FetchResult {
            detail: Some(detail),
            error: None,
        }
    }

    /// Fetch a node by name.
    pub fn fetch_by_name(&self, name: &str, category: Option<NodeCategory>) -> FetchResult {
        let Some(node) = self.graph.find_node_by_name(name, category) else {
            return FetchResult {
                detail: None,
                error: Some(format!("Node '{}' not found", name)),
            };
        };

        self.fetch(node.id)
    }

    /// Fetch a node by path.
    pub fn fetch_by_path(&self, path: &std::path::Path) -> FetchResult {
        let Some(node) = self.graph.find_node_by_path(path) else {
            return FetchResult {
                detail: None,
                error: Some(format!("Node at path '{}' not found", path.display())),
            };
        };

        self.fetch(node.id)
    }

    fn get_incoming_edges(&self, node_id: NodeId) -> Vec<EdgeInfo> {
        self.graph
            .edges_to(node_id)
            .into_iter()
            .filter_map(|(source_id, edge)| {
                let source = self.graph.get_node(source_id)?;
                Some(EdgeInfo {
                    edge_type: edge.edge_type,
                    other_node: NodeSummary::from(source),
                    direction: EdgeDirection::Incoming,
                })
            })
            .collect()
    }

    fn get_outgoing_edges(&self, node_id: NodeId) -> Vec<EdgeInfo> {
        self.graph
            .edges_from(node_id)
            .into_iter()
            .filter_map(|(target_id, edge)| {
                let target = self.graph.get_node(target_id)?;
                Some(EdgeInfo {
                    edge_type: edge.edge_type,
                    other_node: NodeSummary::from(target),
                    direction: EdgeDirection::Outgoing,
                })
            })
            .collect()
    }

    fn get_parents(&self, node_id: NodeId) -> Vec<NodeSummary> {
        self.graph
            .predecessors(node_id)
            .into_iter()
            .filter(|n| {
                self.graph
                    .edge_between(n.id, node_id)
                    .map(|e| e.edge_type == EdgeType::Contains)
                    .unwrap_or(false)
            })
            .map(NodeSummary::from)
            .collect()
    }

    fn get_children(&self, node_id: NodeId) -> Vec<NodeSummary> {
        self.graph
            .children_of(node_id)
            .into_iter()
            .map(NodeSummary::from)
            .collect()
    }

    fn get_functional_centroid(&self, node_id: NodeId) -> Option<NodeSummary> {
        self.graph
            .edges_from(node_id)
            .into_iter()
            .filter(|(_, edge)| edge.edge_type == EdgeType::BelongsToFeature)
            .filter_map(|(target_id, _)| {
                let target = self.graph.get_node(target_id)?;
                Some(NodeSummary::from(target))
            })
            .next()
    }

    fn get_centroid_members(&self, node_id: NodeId) -> Vec<NodeSummary> {
        self.graph
            .centroid_members(node_id)
            .into_iter()
            .map(NodeSummary::from)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_graph() -> RpgGraph {
        let mut graph = RpgGraph::new();

        let file_id = graph.add_node(
            crate::core::Node::new(
                crate::core::NodeId::new(0),
                NodeCategory::File,
                "file",
                "rust",
                "auth.rs",
            )
            .with_path(PathBuf::from("src/auth.rs")),
        );

        let func_id = graph.add_node(
            crate::core::Node::new(
                crate::core::NodeId::new(1),
                NodeCategory::Function,
                "function",
                "rust",
                "login",
            )
            .with_path(PathBuf::from("src/auth.rs"))
            .with_semantic_feature("User login function")
            .with_description("Authenticates a user"),
        );

        graph.add_typed_edge(file_id, func_id, EdgeType::Contains);

        let centroid_id = graph.add_functional_centroid("Auth", "Authentication functionality");

        graph.add_typed_edge(func_id, centroid_id, EdgeType::BelongsToFeature);

        graph
    }

    #[test]
    fn test_fetch_node() {
        let graph = create_test_graph();
        let fetch = FetchNode::new(&graph);

        let result = fetch.fetch(NodeId::new(1));

        assert!(result.detail.is_some());
        let detail = result.detail.unwrap();
        assert_eq!(detail.node.name, "login");
    }

    #[test]
    fn test_fetch_nonexistent_node() {
        let graph = create_test_graph();
        let fetch = FetchNode::new(&graph);

        let result = fetch.fetch(NodeId::new(999));

        assert!(result.detail.is_none());
        assert!(result.error.is_some());
    }

    #[test]
    fn test_fetch_includes_parents() {
        let graph = create_test_graph();
        let fetch = FetchNode::new(&graph);

        let result = fetch.fetch(NodeId::new(1));

        assert!(result.detail.is_some());
        let detail = result.detail.unwrap();
        assert_eq!(detail.parents.len(), 1);
        assert_eq!(detail.parents[0].name, "auth.rs");
    }

    #[test]
    fn test_fetch_includes_centroid() {
        let graph = create_test_graph();
        let fetch = FetchNode::new(&graph);

        let result = fetch.fetch(NodeId::new(1));

        assert!(result.detail.is_some());
        let detail = result.detail.unwrap();
        assert!(detail.functional_centroid.is_some());
        assert_eq!(detail.functional_centroid.unwrap().name, "Auth");
    }

    #[test]
    fn test_fetch_by_name() {
        let graph = create_test_graph();
        let fetch = FetchNode::new(&graph);

        let result = fetch.fetch_by_name("login", Some(NodeCategory::Function));

        assert!(result.detail.is_some());
        assert_eq!(result.detail.unwrap().node.name, "login");
    }

    #[test]
    fn test_node_summary_from_node() {
        let node = crate::core::Node::new(
            crate::core::NodeId::new(42),
            NodeCategory::Function,
            "function",
            "rust",
            "test_func",
        );

        let summary = NodeSummary::from(&node);
        assert_eq!(summary.id, node.id);
        assert_eq!(summary.name, "test_func");
        assert_eq!(summary.category, NodeCategory::Function);
    }
}

//! ExploreRPG Tool - Graph traversal with filtering.
//!
//! Per the paper: "ExploreRPG provides a unified interface for graph traversal,
//! supporting both dependency graph (E_dep) and functional hierarchy (E_feature) views."

use crate::core::{EdgeType, EdgeView, Node, NodeCategory, NodeId, NodeLevel, RpgGraph};

/// Direction for graph traversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraversalDirection {
    /// Follow outgoing edges (dependencies).
    Outgoing,
    /// Follow incoming edges (dependents).
    Incoming,
    /// Follow both directions.
    Both,
}

/// Filter for exploration operations.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[must_use = "ExploreFilter should be used"]
pub struct ExploreFilter {
#[derive(Debug, Clone, Default)]
pub struct ExploreFilter {
    /// Filter by edge view (Functional/Dependency).
    pub edge_view: Option<EdgeView>,
    /// Filter by edge types.
    pub edge_types: Option<Vec<EdgeType>>,
    /// Filter by node categories.
    pub node_categories: Option<Vec<NodeCategory>>,
    /// Filter by node level.
    pub node_level: Option<NodeLevel>,
    /// Maximum depth for traversal.
    pub max_depth: Option<usize>,
    /// Maximum number of nodes to return.
    pub limit: Option<usize>,
}

impl ExploreFilter {
    /// Create a new filter for functional edges only.
    pub fn functional() -> Self {
        Self {
            edge_view: Some(EdgeView::Functional),
            ..Default::default()
        }
    }

    /// Create a new filter for dependency edges only.
    pub fn dependency() -> Self {
        Self {
            edge_view: Some(EdgeView::Dependency),
            ..Default::default()
        }
    }

    /// Set the maximum depth.
    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    /// Set the maximum number of results.
    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Filter to specific node categories.
    pub fn with_categories(mut self, categories: Vec<NodeCategory>) -> Self {
        self.node_categories = Some(categories);
        self
    }

    /// Filter to specific node level.
    pub fn with_level(mut self, level: NodeLevel) -> Self {
        self.node_level = Some(level);
        self
    }
}

/// Result of an exploration operation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "ExploreResult should be used"]
pub struct ExploreResult {
#[derive(Debug, Clone)]
pub struct ExploreResult {
    /// Starting node.
    pub start: NodeId,
    /// Nodes discovered during traversal.
    pub nodes: Vec<Node>,
    /// Edges traversed.
    pub edges: Vec<(NodeId, NodeId, EdgeType)>,
    /// Maximum depth reached.
    pub depth_reached: usize,
}

/// ExploreRPG tool for graph traversal.
pub struct ExploreRPG<'a> {
    graph: &'a RpgGraph,
}

impl<'a> ExploreRPG<'a> {
    /// Create a new ExploreRPG tool.
    #[must_use = "ExploreRPG should be used to explore the graph"]
    pub fn new(graph: &'a RpgGraph) -> Self {
        Self { graph }
    }

    /// Explore from a starting node.
    ///
    /// # Arguments
    /// * `start` - Starting node ID
    /// * `direction` - Traversal direction
    /// * `filter` - Filter configuration
    pub fn explore(
        &self,
        start: NodeId,
        direction: TraversalDirection,
        filter: &ExploreFilter,
    ) -> ExploreResult {
        let mut visited = std::collections::HashSet::new();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut depth_reached = 0;

        // BFS traversal
        let mut queue: std::collections::VecDeque<(NodeId, usize)> =
            std::collections::VecDeque::new();
        queue.push_back((start, 0));
        visited.insert(start);

        while let Some((current_id, depth)) = queue.pop_front() {
            // Check depth limit
            if let Some(max_depth) = filter.max_depth {
                if depth > max_depth {
                    continue;
                }
            }

            depth_reached = depth_reached.max(depth);

            // Get current node
            if let Some(current_node) = self.graph.get_node(current_id) {
                // Apply node filters
                if !self.node_matches_filter(current_node, filter) {
                    continue;
                }

                nodes.push(current_node.clone());
            }

            // Check limit
            if let Some(limit) = filter.limit {
                if nodes.len() >= limit {
                    break;
                }
            }

            // Explore neighbors based on direction
            let neighbors: Vec<(NodeId, EdgeType, bool)> = match direction {
                TraversalDirection::Outgoing => self
                    .graph
                    .edges_from(current_id)
                    .into_iter()
                    .map(|(id, edge)| (id, edge.edge_type, false))
                    .collect(),
                TraversalDirection::Incoming => self
                    .graph
                    .edges_to(current_id)
                    .into_iter()
                    .map(|(id, edge)| (id, edge.edge_type, true))
                    .collect(),
                TraversalDirection::Both => {
                    let mut all = Vec::new();
                    for (id, edge) in self.graph.edges_from(current_id) {
                        all.push((id, edge.edge_type, false));
                    }
                    for (id, edge) in self.graph.edges_to(current_id) {
                        all.push((id, edge.edge_type, true));
                    }
                    all
                }
            };

            for (neighbor_id, edge_type, _is_incoming) in neighbors {
                // Apply edge filters
                if !self.edge_matches_filter(&edge_type, filter) {
                    continue;
                }

                if !visited.contains(&neighbor_id) {
                    visited.insert(neighbor_id);
                    edges.push((current_id, neighbor_id, edge_type));
                    queue.push_back((neighbor_id, depth + 1));
                }
            }
        }

        ExploreResult {
            start,
            nodes,
            edges,
            depth_reached,
        }
    }

    /// Explore the functional hierarchy from a node.
    ///
    /// Returns all nodes in the same functional centroid(s).
    pub fn explore_functional_area(&self, start: NodeId) -> ExploreResult {
        // Find the functional centroid(s) for this node
        let centroids: Vec<NodeId> = self
            .graph
            .edges_from(start)
            .into_iter()
            .filter(|(_, edge)| edge.edge_type == EdgeType::BelongsToFeature)
            .map(|(id, _)| id)
            .collect();

        let mut all_nodes = Vec::new();
        let all_edges = Vec::new();
        let mut visited = std::collections::HashSet::new();

        for centroid_id in centroids {
            // Get all members of this centroid
            for member in self.graph.get_centroid_members(centroid_id) {
                if !visited.contains(&member.id) {
                    visited.insert(member.id);
                    all_nodes.push(member.clone());
                }
            }
        }

        ExploreResult {
            start,
            nodes: all_nodes,
            edges: all_edges,
            depth_reached: 1,
        }
    }

    /// Explore dependency chain from a node.
    ///
    /// Returns all nodes that this node depends on (outgoing Calls/Imports).
    pub fn explore_dependencies(&self, start: NodeId, max_depth: Option<usize>) -> ExploreResult {
        let filter = ExploreFilter {
            edge_view: Some(EdgeView::Dependency),
            edge_types: Some(vec![
                EdgeType::Calls,
                EdgeType::Imports,
                EdgeType::DependsOn,
            ]),
            max_depth,
            ..Default::default()
        };

        self.explore(start, TraversalDirection::Outgoing, &filter)
    }

    /// Explore dependents of a node.
    ///
    /// Returns all nodes that depend on this node (incoming Calls/Imports).
    pub fn explore_dependents(&self, start: NodeId, max_depth: Option<usize>) -> ExploreResult {
        let filter = ExploreFilter {
            edge_view: Some(EdgeView::Dependency),
            edge_types: Some(vec![
                EdgeType::Calls,
                EdgeType::Imports,
                EdgeType::DependsOn,
            ]),
            max_depth,
            ..Default::default()
        };

        self.explore(start, TraversalDirection::Incoming, &filter)
    }

    /// Explore the containment hierarchy.
    ///
    /// Returns all parent/child nodes via Contains edges.
    pub fn explore_containment(
        &self,
        start: NodeId,
        direction: TraversalDirection,
    ) -> ExploreResult {
        let filter = ExploreFilter {
            edge_types: Some(vec![EdgeType::Contains]),
            ..Default::default()
        };

        self.explore(start, direction, &filter)
    }

    /// Get all high-level (V^H) functional centroids.
    pub fn explore_high_level(&self) -> Vec<&Node> {
        self.graph.high_level_nodes().collect()
    }

    /// Get all low-level (V^L) implementation nodes.
    pub fn explore_low_level(&self) -> Vec<&Node> {
        self.graph.low_level_nodes().collect()
    }

    fn node_matches_filter(&self, node: &Node, filter: &ExploreFilter) -> bool {
        if let Some(ref categories) = filter.node_categories {
            if !categories.contains(&node.category) {
                return false;
            }
        }

        if let Some(ref level) = filter.node_level {
            if node.node_level != *level {
                return false;
            }
        }

        true
    }

    fn edge_matches_filter(&self, edge_type: &EdgeType, filter: &ExploreFilter) -> bool {
        // Check edge view filter
        if let Some(ref view) = filter.edge_view {
            if edge_type.view() != *view {
                return false;
            }
        }

        // Check edge type filter
        if let Some(ref types) = filter.edge_types {
            if !types.contains(edge_type) {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_graph() -> RpgGraph {
        let mut graph = RpgGraph::new();

        // Add file
        let file_id = graph.add_node(
            crate::core::Node::new(
                crate::core::NodeId::new(0),
                crate::core::NodeCategory::File,
                "file",
                "rust",
                "main.rs",
            )
            .with_path(PathBuf::from("src/main.rs")),
        );

        // Add functions
        let main_id = graph.add_node(
            crate::core::Node::new(
                crate::core::NodeId::new(1),
                crate::core::NodeCategory::Function,
                "function",
                "rust",
                "main",
            )
            .with_path(PathBuf::from("src/main.rs"))
            .with_node_level(NodeLevel::Low),
        );

        let helper_id = graph.add_node(
            crate::core::Node::new(
                crate::core::NodeId::new(2),
                crate::core::NodeCategory::Function,
                "function",
                "rust",
                "helper",
            )
            .with_path(PathBuf::from("src/main.rs"))
            .with_node_level(NodeLevel::Low),
        );

        // Add Contains edges
        graph.add_typed_edge(file_id, main_id, EdgeType::Contains);
        graph.add_typed_edge(file_id, helper_id, EdgeType::Contains);

        // Add Calls edge
        graph.add_typed_edge(main_id, helper_id, EdgeType::Calls);

        // Add functional centroid
        let centroid_id = graph.add_functional_centroid("Main", "Main entry point functionality");

        // Link to centroid
        graph.add_typed_edge(main_id, centroid_id, EdgeType::BelongsToFeature);
        graph.add_typed_edge(helper_id, centroid_id, EdgeType::BelongsToFeature);

        graph
    }

    #[test]
    fn test_explore_dependencies() {
        let graph = create_test_graph();
        let explore = ExploreRPG::new(&graph);

        let result = explore.explore_dependencies(NodeId::new(1), Some(2));

        assert!(!result.nodes.is_empty());
        assert!(result.nodes.iter().any(|n| n.name == "helper"));
    }

    #[test]
    fn test_explore_containment() {
        let graph = create_test_graph();
        let explore = ExploreRPG::new(&graph);

        let result = explore.explore_containment(NodeId::new(0), TraversalDirection::Outgoing);

        assert!(result.nodes.len() >= 2);
    }

    #[test]
    fn test_explore_functional_area() {
        let graph = create_test_graph();
        let explore = ExploreRPG::new(&graph);

        let result = explore.explore_functional_area(NodeId::new(1));

        // Should include both functions in the same centroid
        assert!(result.nodes.len() >= 1);
    }

    #[test]
    fn test_explore_with_depth_limit() {
        let graph = create_test_graph();
        let explore = ExploreRPG::new(&graph);

        let filter = ExploreFilter::default().with_max_depth(0);
        let result = explore.explore(NodeId::new(1), TraversalDirection::Outgoing, &filter);

        // Should only include the start node
        assert!(result.nodes.len() <= 1);
    }

    #[test]
    fn test_explore_with_limit() {
        let graph = create_test_graph();
        let explore = ExploreRPG::new(&graph);

        let filter = ExploreFilter::default().with_limit(1);
        let result = explore.explore(NodeId::new(0), TraversalDirection::Outgoing, &filter);

        assert!(result.nodes.len() <= 1);
    }

    #[test]
    fn test_explore_high_level() {
        let graph = create_test_graph();
        let explore = ExploreRPG::new(&graph);

        let centroids = explore.explore_high_level();

        assert!(!centroids.is_empty());
        assert!(centroids.iter().all(|n| n.node_level == NodeLevel::High));
    }

    #[test]
    fn test_explore_low_level() {
        let graph = create_test_graph();
        let explore = ExploreRPG::new(&graph);

        let impls = explore.explore_low_level();

        assert!(!impls.is_empty());
        assert!(impls.iter().all(|n| n.node_level == NodeLevel::Low));
    }
}

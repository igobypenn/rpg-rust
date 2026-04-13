//! SearchNode Tool - Semantic search for nodes.
//!
//! Per the paper: "Given a natural language query q, SearchNode(q) returns
//! the top-k most semantically relevant nodes from the graph."

use crate::core::{Node, NodeCategory, NodeLevel, RpgGraph};

/// Configuration for search operations.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    /// Maximum number of results to return.
    pub limit: usize,
    /// Minimum similarity threshold (0.0-1.0).
    pub min_similarity: f32,
    /// Filter by node category (None = all categories).
    pub category_filter: Option<Vec<NodeCategory>>,
    /// Filter by node level (None = all levels).
    pub level_filter: Option<NodeLevel>,
    /// Whether to include high-level (V^H) nodes in results.
    pub include_high_level: bool,
    /// Whether to include low-level (V^L) nodes in results.
    pub include_low_level: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            limit: 10,
            min_similarity: 0.3,
            category_filter: None,
            level_filter: None,
            include_high_level: true,
            include_low_level: true,
        }
    }
}

/// A single search result.
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// The matched node.
    pub node: Node,
    /// Similarity score (0.0-1.0).
    pub score: f32,
    /// Which field produced the best match.
    pub matched_field: String,
}

/// SearchNode tool for semantic node search.
pub struct SearchNode<'a> {
    graph: &'a RpgGraph,
    config: SearchConfig,
}

impl<'a> SearchNode<'a> {
    /// Create a new SearchNode tool with default configuration.
    #[must_use = "SearchNode should be used to perform searches"]
    pub fn new(graph: &'a RpgGraph) -> Self {
        Self {
            graph,
            config: SearchConfig::default(),
        }
    }

    /// Create a new SearchNode tool with custom configuration.
    pub fn with_config(graph: &'a RpgGraph, config: SearchConfig) -> Self {
        Self { graph, config }
    }

    /// Execute a semantic search query.
    ///
    /// # Arguments
    /// * `query` - Natural language query string
    ///
    /// # Returns
    /// Vector of search results sorted by similarity score (descending).
    pub fn search(&self, query: &str) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let query_words: std::collections::HashSet<&str> = query_lower.split_whitespace().collect();

        if query_words.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<SearchResult> = self
            .graph
            .nodes()
            .filter(|node| self.matches_filters(node))
            .filter_map(|node| {
                let (score, field) = self.compute_similarity(node, &query_words);
                if score >= self.config.min_similarity {
                    Some(SearchResult {
                        node: node.clone(),
                        score,
                        matched_field: field,
                    })
                } else {
                    None
                }
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(self.config.limit);

        results
    }

    fn matches_filters(&self, node: &Node) -> bool {
        if !self.config.include_high_level && node.node_level == NodeLevel::High {
            return false;
        }
        if !self.config.include_low_level && node.node_level == NodeLevel::Low {
            return false;
        }
        if let Some(ref level) = self.config.level_filter {
            if node.node_level != *level {
                return false;
            }
        }
        if let Some(ref categories) = self.config.category_filter {
            if !categories.contains(&node.category) {
                return false;
            }
        }
        true
    }

    fn compute_similarity(
        &self,
        node: &Node,
        query_words: &std::collections::HashSet<&str>,
    ) -> (f32, String) {
        let mut best_score = 0.0f32;
        let mut best_field = String::new();

        let name_score = self.jaccard_similarity(&node.name.to_lowercase(), query_words);
        if name_score > best_score {
            best_score = name_score;
            best_field = "name".to_string();
        }

        if let Some(ref feature) = node.semantic_feature {
            let feature_score = self.jaccard_similarity(&feature.to_lowercase(), query_words);
            if feature_score > best_score {
                best_score = feature_score;
                best_field = "semantic_feature".to_string();
            }
        }

        if let Some(ref desc) = node.description {
            let desc_score = self.jaccard_similarity(&desc.to_lowercase(), query_words);
            if desc_score > best_score {
                best_score = desc_score;
                best_field = "description".to_string();
            }
        }

        for feature in &node.features {
            let feature_score = self.jaccard_similarity(&feature.to_lowercase(), query_words);
            if feature_score > best_score {
                best_score = feature_score;
                best_field = "features".to_string();
            }
        }

        (best_score, best_field)
    }

    fn jaccard_similarity(&self, text: &str, query_words: &std::collections::HashSet<&str>) -> f32 {
        crate::utils::jaccard_similarity(text, query_words)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_graph() -> RpgGraph {
        let mut graph = RpgGraph::new();

        graph.add_node(
            crate::core::Node::new(
                crate::core::NodeId::new(0),
                NodeCategory::Function,
                "function",
                "rust",
                "login_user",
            )
            .with_semantic_feature("Authenticates user with credentials and creates session")
            .with_path(PathBuf::from("src/auth/login.rs"))
            .with_node_level(NodeLevel::Low),
        );

        graph.add_node(
            crate::core::Node::new(
                crate::core::NodeId::new(1),
                NodeCategory::Function,
                "function",
                "rust",
                "logout_user",
            )
            .with_semantic_feature("Terminates user session and clears authentication")
            .with_path(PathBuf::from("src/auth/logout.rs"))
            .with_node_level(NodeLevel::Low),
        );

        graph.add_node(
            crate::core::Node::new(
                crate::core::NodeId::new(2),
                NodeCategory::Function,
                "function",
                "rust",
                "process_payment",
            )
            .with_semantic_feature("Processes payment transactions")
            .with_path(PathBuf::from("src/payment/processor.rs"))
            .with_node_level(NodeLevel::Low),
        );

        graph
    }

    #[test]
    fn test_search_by_semantic_feature() {
        let graph = create_test_graph();
        let search = SearchNode::new(&graph);

        let results = search.search("authenticates credentials session");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r
            .node
            .semantic_feature
            .as_ref()
            .map(|f| f.contains("session"))
            .unwrap_or(false)));
    }

    #[test]
    fn test_search_by_name() {
        let graph = create_test_graph();
        let search = SearchNode::new(&graph);

        let results = search.search("payment transactions");

        assert!(!results.is_empty());
        assert!(results.iter().any(|r| r.node.name == "process_payment"));
    }

    #[test]
    fn test_search_with_limit() {
        let graph = create_test_graph();
        let config = SearchConfig {
            limit: 1,
            ..Default::default()
        };
        let search = SearchNode::with_config(&graph, config);

        let results = search.search("authentication session");

        assert!(results.len() <= 1);
    }

    #[test]
    fn test_search_with_min_similarity() {
        let graph = create_test_graph();
        let config = SearchConfig {
            min_similarity: 0.8,
            ..Default::default()
        };
        let search = SearchNode::with_config(&graph, config);

        let results = search.search("xyz123nonexistent");

        assert!(results.is_empty());
    }
}

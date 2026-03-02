//! SearchNode Tool - Semantic search for nodes.
//!
//! Per the paper: "Given a natural language query q, SearchNode(q) returns
//! the top-k most semantically relevant nodes from the graph."

use crate::core::{Node, NodeCategory, NodeId, NodeLevel, RpgGraph};

/// Configuration for search operations.
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
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use = "SearchResult should be used"]
pub struct SearchResult {

/// SearchNode tool for semantic node search.
    /// Create a new SearchNode tool with default configuration.
    #[must_use = "SearchNode should be used to perform searches"]
    pub fn new(graph: &'a RpgGraph) -> Self {
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
        let query_words: std::collections::HashSet<&str> =
            query_lower.split_whitespace().collect();

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

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(self.config.limit);

        results
    }

    /// Search within a specific functional centroid's members.
    pub fn search_in_centroid(&self, query: &str, centroid_id: NodeId) -> Vec<SearchResult> {
        let query_lower = query.to_lowercase();
        let query_words: std::collections::HashSet<&str> =
            query_lower.split_whitespace().collect();

        if query_words.is_empty() {
            return Vec::new();
        }

        let members = self.graph.get_centroid_members(centroid_id);

        let mut results: Vec<SearchResult> = members
            .into_iter()
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
        // Check level filter
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

        // Check category filter
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

        // Check name similarity
        let name_score = self.jaccard_similarity(&node.name.to_lowercase(), query_words);
        if name_score > best_score {
            best_score = name_score;
            best_field = "name".to_string();
        }

        // Check semantic feature similarity
        if let Some(ref feature) = node.semantic_feature {
            let feature_score = self.jaccard_similarity(&feature.to_lowercase(), query_words);
            if feature_score > best_score {
                best_score = feature_score;
                best_field = "semantic_feature".to_string();
            }
        }

        // Check description similarity
        if let Some(ref desc) = node.description {
            let desc_score = self.jaccard_similarity(&desc.to_lowercase(), query_words);
            if desc_score > best_score {
                best_score = desc_score;
                best_field = "description".to_string();
            }
        }

        // Check features similarity
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

        // Add some nodes with semantic features
        let _auth_centroid = graph.add_functional_centroid(
            "Authentication",
            "Handles user authentication login logout session management",
        );

        graph.add_node(
            crate::core::Node::new(
                crate::core::NodeId::new(0),
                crate::core::NodeCategory::Function,
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
                crate::core::NodeCategory::Function,
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
                crate::core::NodeCategory::Function,
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

        // Query that matches words in the semantic feature
        let results = search.search("authenticates credentials session");

        assert!(!results.is_empty());
        // Should match login_user because its semantic_feature contains these words
        assert!(results.iter().any(|r| 
            r.node.semantic_feature.as_ref().map(|f| f.contains("session")).unwrap_or(false)
        ));
    }

    #[test]
    fn test_search_by_name() {
        let graph = create_test_graph();
        let search = SearchNode::new(&graph);

        // Query using words from the semantic feature
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

        // Query that matches multiple nodes but should be limited
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

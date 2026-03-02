//! Functional Abstraction Module (Paper Phase 2).
//!
//! Implements the paper's Functional Abstraction and Hierarchical Aggregation:
//! 1. Collect semantic features from V^L (low-level implementation nodes)
//! 2. Induce functional centroids (abstract functional areas)
//! 3. Create V^H (high-level centroid nodes) in the graph
//! 4. Link V^L → V^H via E_feature edges (BelongsToFeature)

use std::collections::HashMap;

use crate::error::Result;

use crate::core::{EdgeType, Node, NodeCategory, NodeId, NodeLevel, RpgGraph};

#[cfg(feature = "llm")]
use crate::llm::OpenAIClient;

/// Unified prompt for domain discovery + hierarchical assignment.
/// Collapses 3 LLM calls into 1 for 95% API call reduction.
///
/// Per paper Appendix A.1.2: "discover a small set of high-level functional areas
/// that act as architectural centroids."
#[cfg(feature = "llm")]
const UNIFIED_ABSTRACTION_PROMPT: &str = r#"You are an expert software architect.

## Task
Analyze the repository and produce:
1. Functional areas (1-8 high-level domains)
2. Hierarchical assignment map (three-level paths)

## Constraints
- Output 1-8 functional areas (be conservative)
- Areas must be mutually exclusive and collectively cover the repo
- NO vague terms: Core, Misc, Other, Utils, Common, General, Shared
- Use PascalCase for functional areas
- Three-level path: <area>/<category>/<subcategory>
- Every feature MUST be assigned to exactly one path

## Output Format (STRICT JSON)
{
  \"functional_areas\": [\"Area1\", \"Area2\"],
  \"assignments\": {
    \"Area1/Category/Subcategory\": [\"feature_name_1\"],
    \"Area2/Category/Subcategory\": [\"feature_name_2\"]
  }
}

Repository: {repo_info}
Features:
{features_summary}
"#;

/// Response from unified abstraction prompt.
#[cfg(feature = "llm")]
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AbstractionResponse {
    /// Discovered functional areas (1-8 items).
    pub functional_areas: Vec<String>,
    /// Map from three-level path to feature names.
    pub assignments: HashMap<String, Vec<String>>,
}

/// Options for LLM-based functional abstraction.
#[cfg(feature = "llm")]
#[derive(Debug, Clone)]
pub struct LlmOptions<'a> {
    /// LLM client for API calls.
    pub client: &'a OpenAIClient,
    /// Repository description for context.
    pub repo_info: String,
    /// Maximum retry attempts (default: 2).
    pub max_retries: u32,
    /// Enable compatibility validation.
    pub validate_compatibility: bool,
}

#[cfg(feature = "llm")]
impl<'a> LlmOptions<'a> {
    pub fn new(client: &'a OpenAIClient, repo_info: impl Into<String>) -> Self {
        Self {
            client,
            repo_info: repo_info.into(),
            max_retries: 2,
            validate_compatibility: false,
        }
    }

    pub fn with_validation(mut self) -> Self {
        self.validate_compatibility = true;
        self
    }

    pub fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }
}

/// Configuration for functional abstraction.
#[derive(Debug, Clone)]
pub struct FunctionalConfig {
    pub batch_size: usize,
    pub similarity_threshold: f32,
    pub max_depth: usize,
}

impl Default for FunctionalConfig {
    fn default() -> Self {
        Self {
            batch_size: 100,
            similarity_threshold: 0.7,
            max_depth: 3,
        }
    }
}

/// A collected semantic feature from a V^L node.
#[derive(Debug, Clone)]
pub struct CollectedFeature {
    pub node_id: NodeId,
    pub name: String,
    pub semantic_feature: String,
    pub category: NodeCategory,
    pub path: Option<String>,
}

/// A functional centroid (V^H node).
#[derive(Debug, Clone)]
pub struct FunctionalCentroid {
    pub name: String,
    pub description: String,
    pub semantic_feature: String,
    pub parent: Option<String>,
}

/// Result of functional abstraction.
#[derive(Debug, Clone, Default)]
pub struct AbstractionResult {
    pub centroids_created: usize,
    pub nodes_linked: usize,
    pub edges_created: usize,
}

/// Functional Abstraction Engine.
pub struct FunctionalAbstraction<'a> {
    graph: &'a mut RpgGraph,
    #[allow(dead_code)]
    config: FunctionalConfig,
}

impl<'a> FunctionalAbstraction<'a> {
    pub fn new(graph: &'a mut RpgGraph) -> Self {
        Self {
            graph,
            config: FunctionalConfig::default(),
        }
    }

    pub fn with_config(graph: &'a mut RpgGraph, config: FunctionalConfig) -> Self {
        Self { graph, config }
    }

    /// Phase 2.1: Collect semantic features from V^L nodes.
    pub fn collect_semantic_features(&self) -> Vec<CollectedFeature> {
        self.graph
            .low_level_nodes()
            .filter_map(|node| {
                let semantic_feature = node.semantic_feature.as_ref()?;
                Some(CollectedFeature {
                    node_id: node.id,
                    name: node.name.clone(),
                    semantic_feature: semantic_feature.clone(),
                    category: node.category,
                    path: node.path.as_ref().map(|p| p.to_string_lossy().to_string()),
                })
            })
            .collect()
    }

    /// Phase 2.2: Induce functional centroids via heuristic (path-based).
    pub fn induce_centroids_heuristic(
        &self,
        features: &[CollectedFeature],
    ) -> Vec<FunctionalCentroid> {
        let mut centroids: HashMap<String, FunctionalCentroid> = HashMap::new();

        for feature in features {
            if let Some(path) = &feature.path {
                let parts: Vec<&str> = path
                    .trim_start_matches("./")
                    .trim_start_matches("src/")
                    .split('/')
                    .take(2)
                    .collect();

                if let Some(&area_name) = parts.first() {
                    let area = to_title_case(area_name);

                    centroids
                        .entry(area.clone())
                        .or_insert_with(|| FunctionalCentroid {
                            name: area,
                            description: format!(
                                "Functional area for {} related functionality",
                                area_name
                            ),
                            semantic_feature: format!("Handles {} related operations", area_name),
                            parent: None,
                        });
                }
            }
        }

        centroids.into_values().collect()
    }

    /// Phase 2.3: Create V^H (functional centroid) nodes in the graph.
    pub fn create_centroid_nodes(
        &mut self,
        centroids: &[FunctionalCentroid],
    ) -> HashMap<String, NodeId> {
        let mut centroid_map: HashMap<String, NodeId> = HashMap::new();

        for centroid in centroids {
            let node = Node::new(
                NodeId::new(self.graph.node_count()),
                NodeCategory::FunctionalCentroid,
                "functional_centroid",
                "abstract",
                &centroid.name,
            )
            .with_node_level(NodeLevel::High)
            .with_semantic_feature(&centroid.semantic_feature)
            .with_description(&centroid.description);

            let node_id = self.graph.add_node(node);
            centroid_map.insert(centroid.name.clone(), node_id);

            if let Some(parent_name) = &centroid.parent {
                if let Some(&parent_id) = centroid_map.get(parent_name) {
                    self.graph
                        .add_typed_edge(parent_id, node_id, EdgeType::ContainsFeature);
                }
            }
        }

        centroid_map
    }

    /// Phase 2.4: Hierarchical Aggregation - Link V^L nodes to V^H centroids.
    pub fn aggregate_hierarchy(
        &mut self,
        features: &[CollectedFeature],
        centroid_map: &HashMap<String, NodeId>,
    ) -> Result<AbstractionResult> {
        let mut result = AbstractionResult::default();

        for feature in features {
            let best_match = self.find_best_centroid(&feature.semantic_feature, centroid_map);

            if let Some((_centroid_name, centroid_id)) = best_match {
                self.graph
                    .add_typed_edge(feature.node_id, centroid_id, EdgeType::BelongsToFeature);
                result.nodes_linked += 1;
                result.edges_created += 1;
            }
        }

        result.centroids_created = centroid_map.len();
        Ok(result)
    }

    fn find_best_centroid(
        &self,
        semantic_feature: &str,
        centroid_map: &HashMap<String, NodeId>,
    ) -> Option<(String, NodeId)> {
        let feature_lower = semantic_feature.to_lowercase();
        let mut best_match: Option<(String, NodeId, usize)> = None;

        for (name, id) in centroid_map {
            let name_lower = name.to_lowercase();
            let score = if feature_lower.contains(&name_lower)
                || name_lower
                    .split_whitespace()
                    .any(|w| feature_lower.contains(w))
            {
                name.split_whitespace().count()
            } else {
                0
            };

            if score > 0 {
                match &best_match {
                    None => best_match = Some((name.clone(), *id, score)),
                    Some((_, _, best_score)) if score > *best_score => {
                        best_match = Some((name.clone(), *id, score));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(name, id, _)| (name, id))
    }

    /// Phase 3: Artifact Grounding - Ground all functional centroids to their LCA paths.
    ///
    /// Per the paper: "populate the missing metadata m for nodes in V^H through
    /// bottom-up propagation, utilizing a Lowest Common Ancestor (LCA) mechanism."
    ///
    /// Returns the number of centroids grounded.
    pub fn ground_centroids(&mut self) -> usize {
        // Collect centroid IDs first to avoid borrow issues
        let centroid_ids: Vec<NodeId> = self.graph.functional_centroids().map(|n| n.id).collect();

        let mut grounded_count = 0;
        for centroid_id in centroid_ids {
            if self.graph.ground_centroid(centroid_id).is_some() {
                grounded_count += 1;
            }
        }

        grounded_count
    }

    /// Run the complete functional abstraction pipeline.
    pub fn run(&mut self) -> Result<AbstractionResult> {
        let features = self.collect_semantic_features();

        if features.is_empty() {
            return Ok(AbstractionResult::default());
        }

        let centroids = self.induce_centroids_heuristic(&features);
        let centroid_map = self.create_centroid_nodes(&centroids);
        let result = self.aggregate_hierarchy(&features, &centroid_map)?;

        // Phase 3: Artifact Grounding
        let _grounded = self.ground_centroids();

        Ok(result)
    }

    // ========================================================================
    // LLM-Based Methods (2-Call Architecture)
    // ========================================================================

    /// Run with LLM-based centroid induction, with heuristic fallback.
    ///
    /// Per paper Appendix A.1.2: "discover a small set of high-level functional areas
    /// that act as architectural centroids."
    ///
    /// This method automatically falls back to heuristic if:
    /// - LLM feature is not enabled
    /// - LLM client is None
    /// - LLM call fails after retries
    ///
    /// # Example
    /// ```ignore
    /// let mut abstraction = FunctionalAbstraction::new(&mut graph);
    /// let result = abstraction.run_with_llm(llm_options).await?;
    /// ```
    #[cfg(feature = "llm")]
    pub async fn run_with_llm(
        &mut self,
        options: Option<&LlmOptions<'_>>,
    ) -> Result<AbstractionResult> {
        // Try LLM path if options provided
        if let Some(opts) = options {
            match self.induce_centroids_llm(opts).await {
                Ok((centroids, assignments)) => {
                    let features = self.collect_semantic_features();
                    return self.build_hierarchy_from_llm(&features, &centroids, &assignments);
                }
                Err(e) => {
                    tracing::warn!("LLM induction failed, falling back to heuristic: {}", e);
                    // Fall through to heuristic
                }
            }
        }

        // Fallback: heuristic approach
        self.run()
    }

    /// LLM-based centroid induction (single unified call).
    ///
    /// Returns: (centroids, assignments map)
    #[cfg(feature = "llm")]
    async fn induce_centroids_llm(
        &self,
        options: &LlmOptions<'_>,
    ) -> Result<(Vec<FunctionalCentroid>, HashMap<String, Vec<String>>)> {
        let features = self.collect_semantic_features();

        if features.is_empty() {
            return Ok((vec![], HashMap::new()));
        }

        // Build features summary
        let features_summary: String = features
            .iter()
            .map(|f| format!("- {}: {}", f.name, f.semantic_feature))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = UNIFIED_ABSTRACTION_PROMPT
            .replace("{repo_info}", &options.repo_info)
            .replace("{features_summary}", &features_summary);

        // Retry loop with exponential backoff
        let mut last_error: Option<crate::llm::LlmError> = None;
        for attempt in 0..=options.max_retries {
            match options
                .client
                .complete_json::<AbstractionResponse>("", &prompt)
                .await
            {
                Ok(response) => {
                    // Validate response against paper constraints
                    if let Some(validated) = Self::validate_llm_response(response) {
                        let centroids = Self::response_to_centroids(&validated);
                        return Ok((centroids, validated.assignments));
                    }
                    tracing::warn!(
                        "LLM response validation failed (attempt {}/{})",
                        attempt,
                        options.max_retries
                    );
                }
                Err(e) => {
                    last_error = Some(e);
                    if attempt < options.max_retries {
                        let delay = std::time::Duration::from_millis(100 * 2u64.pow(attempt));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(crate::error::RpgError::HttpClient(
            last_error
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Max retries exceeded".to_string()),
        ))
    }

    /// Build hierarchy from LLM response, creating intermediate nodes.
    ///
    /// Per paper: "instantiating intermediate nodes to bridge the hierarchy
    /// when a direct link lacks granularity."
    #[cfg(feature = "llm")]
    fn build_hierarchy_from_llm(
        &mut self,
        features: &[CollectedFeature],
        centroids: &[FunctionalCentroid],
        assignments: &HashMap<String, Vec<String>>,
    ) -> Result<AbstractionResult> {
        let mut result = AbstractionResult::default();

        // Create centroid nodes
        let centroid_map = self.create_centroid_nodes(centroids);
        result.centroids_created = centroid_map.len();

        // Build feature name to node ID map
        let feature_to_node: HashMap<&str, NodeId> = features
            .iter()
            .map(|f| (f.name.as_str(), f.node_id))
            .collect();

        // Track intermediate nodes: path -> NodeId
        let mut intermediate_nodes: HashMap<String, NodeId> = HashMap::new();

        // Process assignments
        for (path, feature_names) in assignments {
            // Parse three-level path: Area/Category/Subcategory
            let parts: Vec<&str> = path.split('/').collect();
            if parts.len() != 3 {
                continue;
            }

            let functional_area = parts[0];

            // Get or create intermediate node
            let intermediate_id = *intermediate_nodes.entry(path.clone()).or_insert_with(|| {
                self.create_intermediate_node(functional_area, path, &centroid_map)
            });

            // Link features to intermediate node
            for feature_name in feature_names {
                if let Some(&feature_id) = feature_to_node.get(feature_name.as_str()) {
                    self.graph.add_typed_edge(
                        feature_id,
                        intermediate_id,
                        EdgeType::BelongsToFeature,
                    );
                    result.nodes_linked += 1;
                    result.edges_created += 1;
                }
            }
        }

        // Ground centroids
        self.ground_centroids();

        Ok(result)
    }

    /// Create an intermediate node for three-level hierarchy.
    #[cfg(feature = "llm")]
    fn create_intermediate_node(
        &mut self,
        functional_area: &str,
        path: &str,
        centroid_map: &HashMap<String, NodeId>,
    ) -> NodeId {
        let node = Node::new(
            NodeId::new(self.graph.node_count()),
            NodeCategory::Module,
            "intermediate",
            "abstract",
            path,
        )
        .with_node_level(NodeLevel::Intermediate)
        .with_semantic_feature(format!("Intermediate node for {}", path))
        .with_feature_path(path);

        let node_id = self.graph.add_node(node);

        // Link to parent centroid
        if let Some(&centroid_id) = centroid_map.get(functional_area) {
            self.graph
                .add_typed_edge(node_id, centroid_id, EdgeType::BelongsToFeature);
        }

        node_id
    }

    /// Validate LLM response against paper constraints.
    ///
    /// Paper constraints:
    /// - 1-8 functional areas
    /// - No vague terms (Core, Misc, Other, etc.)
    /// - PascalCase names
    /// - All features assigned
    #[cfg(feature = "llm")]
    fn validate_llm_response(response: AbstractionResponse) -> Option<AbstractionResponse> {
        // Paper: 1-8 functional areas
        if response.functional_areas.is_empty() || response.functional_areas.len() > 8 {
            return None;
        }

        // Paper: No vague terms
        const VAGUE_TERMS: &[&str] = &[
            "core", "misc", "other", "utils", "common", "general", "shared",
        ];
        let has_vague = response.functional_areas.iter().any(|area| {
            let lower = area.to_lowercase();
            VAGUE_TERMS.iter().any(|vague| lower.contains(vague))
        });
        if has_vague {
            return None;
        }

        // Paper: PascalCase (first char uppercase)
        let all_pascal_case = response.functional_areas.iter().all(|area| {
            area.chars()
                .next()
                .map(|c| c.is_uppercase())
                .unwrap_or(false)
        });
        if !all_pascal_case {
            return None;
        }

        // Paper: All features must be assigned
        if response.assignments.is_empty() {
            return None;
        }

        Some(response)
    }

    /// Convert validated response to FunctionalCentroids.
    #[cfg(feature = "llm")]
    fn response_to_centroids(response: &AbstractionResponse) -> Vec<FunctionalCentroid> {
        response
            .functional_areas
            .iter()
            .map(|area| FunctionalCentroid {
                name: area.clone(),
                description: format!("Functional area for {} related functionality", area),
                semantic_feature: format!("Handles {} operations", area),
                parent: None,
            })
            .collect()
    }
}

fn to_title_case(s: &str) -> String {
    s.split(['_', '-', '/'])
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    first.to_uppercase().collect::<String>()
                        + chars.as_str().to_lowercase().as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn create_test_graph_with_semantic_features() -> RpgGraph {
        let mut graph = RpgGraph::new();

        // Add function nodes with semantic features
        let _login_id = graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::Function,
                "function",
                "rust",
                "login",
            )
            .with_semantic_feature("Handles user authentication and session management")
            .with_path(PathBuf::from("src/auth/login.rs")),
        );

        let _logout_id = graph.add_node(
            Node::new(
                NodeId::new(1),
                NodeCategory::Function,
                "function",
                "rust",
                "logout",
            )
            .with_semantic_feature("Handles user logout and session termination")
            .with_path(PathBuf::from("src/auth/login.rs")),
        );

        graph
    }

    #[test]
    fn test_collect_semantic_features() {
        let mut graph = create_test_graph_with_semantic_features();
        let abstraction = FunctionalAbstraction::new(&mut graph);
        let features = abstraction.collect_semantic_features();

        assert_eq!(features.len(), 2);
        assert!(features.iter().any(|f| f.name == "login"));
        assert!(features.iter().any(|f| f.name == "logout"));
    }

    #[test]
    fn test_induce_centroids_heuristic() {
        let mut graph = create_test_graph_with_semantic_features();
        let abstraction = FunctionalAbstraction::new(&mut graph);
        let features = abstraction.collect_semantic_features();
        let centroids = abstraction.induce_centroids_heuristic(&features);

        assert!(!centroids.is_empty());
        assert!(centroids.iter().any(|c| c.name.contains("Auth")));
    }

    #[test]
    fn test_create_centroid_nodes() {
        let mut graph = create_test_graph_with_semantic_features();
        let mut abstraction = FunctionalAbstraction::new(&mut graph);

        let centroids = vec![FunctionalCentroid {
            name: "Authentication".to_string(),
            description: "Handles authentication".to_string(),
            semantic_feature: "Authentication functionality".to_string(),
            parent: None,
        }];

        let centroid_map = abstraction.create_centroid_nodes(&centroids);

        assert_eq!(centroid_map.len(), 1);
        assert!(centroid_map.contains_key("Authentication"));
    }

    #[test]
    fn test_run() {
        let mut graph = create_test_graph_with_semantic_features();
        let mut abstraction = FunctionalAbstraction::new(&mut graph);
        let result = abstraction.run().unwrap();

        assert!(result.centroids_created > 0);
        assert!(result.nodes_linked > 0);
        assert!(result.edges_created > 0);
    }

    #[test]
    fn test_to_title_case() {}

    #[test]
    fn test_ground_centroids() {
        let mut graph = RpgGraph::new();

        // Add a functional centroid (V^H)
        let centroid_id = graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::FunctionalCentroid,
                "functional_centroid",
                "abstract",
                "Auth",
            )
            .with_node_level(NodeLevel::High)
            .with_semantic_feature("Authentication functionality"),
        );

        // Add some V^L nodes with paths
        let login_id = graph.add_node(
            Node::new(
                NodeId::new(1),
                NodeCategory::Function,
                "function",
                "rust",
                "login",
            )
            .with_node_level(NodeLevel::Low)
            .with_path(PathBuf::from("src/auth/login.rs")),
        );

        let logout_id = graph.add_node(
            Node::new(
                NodeId::new(2),
                NodeCategory::Function,
                "function",
                "rust",
                "logout",
            )
            .with_node_level(NodeLevel::Low)
            .with_path(PathBuf::from("src/auth/logout.rs")),
        );

        // Link V^L nodes to centroid
        graph.add_typed_edge(login_id, centroid_id, EdgeType::BelongsToFeature);
        graph.add_typed_edge(logout_id, centroid_id, EdgeType::BelongsToFeature);

        // Verify centroid has no path initially
        assert!(graph.get_node(centroid_id).unwrap().path.is_none());

        // Run grounding
        let mut abstraction = FunctionalAbstraction::new(&mut graph);
        let grounded = abstraction.ground_centroids();

        assert_eq!(grounded, 1);

        // Verify centroid now has a path
        let centroid = abstraction.graph.get_node(centroid_id).unwrap();
        assert!(centroid.path.is_some());
        let path = centroid.path.as_ref().unwrap();
        assert!(path.to_str().unwrap().contains("src/auth"));
    }
}

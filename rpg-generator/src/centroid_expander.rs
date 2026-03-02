//! Centroid Expander - Forward path generation from functional centroids.
//!
//! This module implements the inverse of FunctionalAbstraction:
//! - **FunctionalAbstraction** (encoder): V^L nodes → cluster → induce V^H centroids
//! - **CentroidExpander** (generator): V^H centroids → expand → generate V^L nodes
//!
//! This enables the forward path where planned functional areas (centroids)
//! are expanded into concrete implementation nodes.

use std::collections::HashMap;

use rpg_encoder::{
    RpgGraph, Node, NodeId, NodeCategory, NodeLevel, EdgeType,
    FeatureTree, FeatureNode,
};
use rpg_encoder::encoder::{FunctionalCentroid, CollectedFeature};

use crate::error::{GeneratorError, Result};

/// Result of expanding centroids into implementation nodes.
#[derive(Debug, Clone, Default)]
pub struct ExpansionResult {
    /// Number of V^L nodes created
    pub nodes_created: usize,
    /// Number of edges created (BelongsToFeature links)
    pub edges_created: usize,
    /// Number of centroids expanded
    pub centroids_expanded: usize,
    /// Map from centroid name to created node IDs
    pub created_nodes: HashMap<String, Vec<NodeId>>,
}

impl ExpansionResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes_created == 0
    }
}

/// Configuration for centroid expansion.
#[derive(Debug, Clone)]
pub struct ExpansionConfig {
    /// Maximum depth of expansion (default: 3)
    pub max_depth: usize,
    /// Maximum nodes per centroid (default: 10)
    pub max_nodes_per_centroid: usize,
    /// Whether to create intermediate category nodes
    pub create_categories: bool,
    /// Whether to infer data structures
    pub infer_data_structures: bool,
}

impl Default for ExpansionConfig {
    fn default() -> Self {
        Self {
            max_depth: 3,
            max_nodes_per_centroid: 10,
            create_categories: true,
            infer_data_structures: true,
        }
    }
}

/// Centroid Expander - expands V^H centroids into V^L implementation nodes.
///
/// This is the inverse operation of FunctionalAbstraction:
/// - Given a functional centroid (e.g., "Authentication"), generate the
///   functions, types, and modules that should implement it.
///
/// # Example
///
/// ```ignore
/// use rpg_generator::CentroidExpander;
/// use rpg_encoder::RpgGraph;
///
/// let mut graph = RpgGraph::new();
/// // ... add centroids ...
///
/// let mut expander = CentroidExpander::new(&mut graph);
/// let result = expander.expand_all();
///
/// println!("Created {} nodes from {} centroids",
///     result.nodes_created, result.centroids_expanded);
/// ```
pub struct CentroidExpander<'a> {
    graph: &'a mut RpgGraph,
    config: ExpansionConfig,
}

impl<'a> CentroidExpander<'a> {
    /// Create a new centroid expander.
    pub fn new(graph: &'a mut RpgGraph) -> Self {
        Self {
            graph,
            config: ExpansionConfig::default(),
        }
    }

    /// Create with custom configuration.
    pub fn with_config(graph: &'a mut RpgGraph, config: ExpansionConfig) -> Self {
        Self { graph, config }
    }

    /// Expand a single centroid into V^L implementation nodes.
    ///
    /// This method analyzes the centroid's semantic feature and generates
    /// appropriate function/type/module nodes based on common patterns.
    pub fn expand_centroid(&mut self, centroid: &FunctionalCentroid) -> Result<Vec<NodeId>> {
        let mut created_nodes = Vec::new();

        // Parse semantic feature to infer what to generate
        let inferred_units = self.infer_units_from_semantic(&centroid.semantic_feature);

        for unit in inferred_units {
            // Limit nodes per centroid
            if created_nodes.len() >= self.config.max_nodes_per_centroid {
                break;
            }

            let node = self.create_node_for_unit(&unit, centroid);
            let node_id = self.graph.add_node(node);

            created_nodes.push(node_id);
        }

        Ok(created_nodes)
    }

    /// Expand all centroids in the graph.
    ///
    /// Finds all V^H (High-level) nodes and expands each into V^L nodes.
    pub fn expand_all(&mut self) -> Result<ExpansionResult> {
        let mut result = ExpansionResult::new();

        // Collect centroid IDs first (to avoid borrow issues)
        let centroid_ids: Vec<NodeId> = self.graph.functional_centroids().map(|n| n.id).collect();

        for centroid_id in centroid_ids {
            // Get centroid info
            let centroid_node = self.graph.get_node(centroid_id).ok_or_else(|| {
                GeneratorError::ExpansionFailed(format!("Node {:?} not found", centroid_id))
            })?;

            // Build FunctionalCentroid from node
            let centroid = FunctionalCentroid {
                name: centroid_node.name.clone(),
                description: centroid_node.description.clone().unwrap_or_default(),
                semantic_feature: centroid_node.semantic_feature.clone().unwrap_or_default(),
                parent: None,
            };

            // Expand this centroid
            let created = self.expand_centroid(&centroid)?;

            // Create BelongsToFeature edges
            for node_id in &created {
                self.graph
                    .add_typed_edge(*node_id, centroid_id, EdgeType::BelongsToFeature);
                result.edges_created += 1;
            }

            result.nodes_created += created.len();
            result.centroids_expanded += 1;
            result.created_nodes.insert(centroid.name.clone(), created);
        }

        tracing::info!(
            "Centroid expansion complete: {} nodes from {} centroids",
            result.nodes_created,
            result.centroids_expanded
        );

        Ok(result)
    }

    /// Infer implementation units from semantic feature description.
    ///
    /// Uses heuristics based on common patterns:
    /// - "handle X" → function handle_x
    /// - "manage Y" → struct YManager + methods
    /// - "validate Z" → function validate_z
    fn infer_units_from_semantic(&self, semantic_feature: &str) -> Vec<InferredUnit> {
        let mut units = Vec::new();
        let lower = semantic_feature.to_lowercase();

        // Pattern: "handles X" or "handle X" → function
        if lower.contains("handle") || lower.contains("process") {
            units.push(InferredUnit {
                name: "handler".to_string(),
                kind: UnitKind::Function,
                description: format!("Main handler for {}", semantic_feature),
            });
        }

        // Pattern: "manage X" → manager struct + methods
        if lower.contains("manage") || lower.contains("coordinate") {
            units.push(InferredUnit {
                name: "Manager".to_string(),
                kind: UnitKind::Struct,
                description: format!("Manager for {}", semantic_feature),
            });
            units.push(InferredUnit {
                name: "new".to_string(),
                kind: UnitKind::Function,
                description: "Create new manager instance".to_string(),
            });
        }

        // Pattern: "validate X" → validation function
        if lower.contains("validate") || lower.contains("check") {
            units.push(InferredUnit {
                name: "validate".to_string(),
                kind: UnitKind::Function,
                description: format!("Validation for {}", semantic_feature),
            });
        }

        // Pattern: "store X" or "persist X" → data structure
        if (lower.contains("store") || lower.contains("persist") || lower.contains("save"))
            && self.config.infer_data_structures {
                units.push(InferredUnit {
                    name: "Data".to_string(),
                    kind: UnitKind::Struct,
                    description: format!("Data structure for {}", semantic_feature),
                });
            }

        // Pattern: "compute X" or "calculate X" → function
        if lower.contains("compute") || lower.contains("calculate") {
            units.push(InferredUnit {
                name: "compute".to_string(),
                kind: UnitKind::Function,
                description: format!("Computation for {}", semantic_feature),
            });
        }

        // Default: always add a main function if nothing else
        if units.is_empty() {
            units.push(InferredUnit {
                name: "main".to_string(),
                kind: UnitKind::Function,
                description: format!("Main function for {}", semantic_feature),
            });
        }

        units
    }

    /// Create a graph node for an inferred unit.
    fn create_node_for_unit(&self, unit: &InferredUnit, centroid: &FunctionalCentroid) -> Node {
        let category = match unit.kind {
            UnitKind::Function => NodeCategory::Function,
            UnitKind::Struct => NodeCategory::Type,
            UnitKind::Enum => NodeCategory::Type,
            UnitKind::Trait => NodeCategory::Type,
            UnitKind::Module => NodeCategory::Module,
        };

        let full_name = format!(
            "{}::{}",
            centroid.name.to_lowercase().replace(' ', "_"),
            unit.name
        );

        Node::new(
            NodeId::new(self.graph.node_count()),
            category,
            unit.kind.as_str(),
            "rust", // Default language
            &full_name,
        )
        .with_node_level(NodeLevel::Low)
        .with_description(&unit.description)
        .with_semantic_feature(format!("{} - {}", centroid.name, unit.description))
    }

    /// Expand from a feature tree (alternative entry point).
    ///
    /// Creates centroids from feature tree, then expands them.
    pub fn expand_from_features(
        &mut self,
        features: &[CollectedFeature],
    ) -> Result<ExpansionResult> {
        // Group features by category/path to create centroids
        let mut centroid_map: HashMap<String, Vec<&CollectedFeature>> = HashMap::new();

        for feature in features {
            let category = feature
                .path
                .as_ref()
                .map(|p: &String| p.split('/').next().unwrap_or("core").to_string())
                .unwrap_or_else(|| "core".to_string());

            centroid_map.entry(category).or_default().push(feature);
        }

        // Create centroid nodes
        for (category, feature_list) in &centroid_map {
            let semantic_features: Vec<&str> = feature_list
                .iter()
                .map(|f| f.semantic_feature.as_str())
                .collect();

            let centroid = Node::new(
                NodeId::new(self.graph.node_count()),
                NodeCategory::FunctionalCentroid,
                "functional_centroid",
                "abstract",
                category,
            )
            .with_node_level(NodeLevel::High)
            .with_semantic_feature(semantic_features.join("; "));

            self.graph.add_node(centroid);
        }

        // Now expand all centroids
        self.expand_all()
    }
}

/// Inferred implementation unit from semantic analysis.
#[derive(Debug, Clone)]
struct InferredUnit {
    name: String,
    kind: UnitKind,
    description: String,
}

/// Kind of implementation unit.
/// TODO: Enum/Trait/Module variants reserved for future expansion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
enum UnitKind {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
}

impl UnitKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Trait => "trait",
            Self::Module => "module",
        }
    }
}



/// Create a planned RpgGraph from a FeatureTree.
///
/// This converts the hierarchical FeatureTree into an RpgGraph with
/// functional centroids, ready for expansion or verification.
pub fn create_planned_graph_from_features(feature_tree: &FeatureTree) -> RpgGraph {
    let mut graph = RpgGraph::new();
    
    // Create repository root node
    let root_name = &feature_tree.root.name;
    let repo_node = Node::new(
        NodeId::new(0), // Placeholder, will be replaced by graph.add_node
        NodeCategory::Repository,
        "repository",
        "unknown",
        root_name.clone(),
    ).with_description(format!("Planned repository: {}", root_name));
    
    let repo_id = graph.add_node(repo_node);
    
    // Recursively add features as functional centroids
    fn add_feature_as_centroid(
        graph: &mut RpgGraph,
        parent_id: NodeId,
        node: &FeatureNode,
    ) {
        // Create a functional centroid for this feature
        let centroid = Node::new(
            NodeId::new(0), // Placeholder, will be replaced
            NodeCategory::FunctionalCentroid,
            "feature",
            "unknown",
            node.name.clone(),
        )
        .with_node_level(NodeLevel::High)
        .with_description(node.description.clone().unwrap_or_default())
        .with_semantic_feature(node.features.join("; "));
        
        let centroid_id = graph.add_node(centroid);
        graph.add_typed_edge(centroid_id, parent_id, EdgeType::BelongsToFeature);
        
        // Add child features
        for child in &node.children {
            add_feature_as_centroid(graph, centroid_id, child);
        }
    }
    
    // Process root's children
    for child in &feature_tree.root.children {
        add_feature_as_centroid(&mut graph, repo_id, child);
    }
    
    tracing::info!(
        "Created planned RPG with {} nodes from FeatureTree",
        graph.node_count()
    );
    
    graph
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpg_encoder::Node;

    fn create_test_graph_with_centroid() -> RpgGraph {
        let mut graph = RpgGraph::new();

        let centroid = Node::new(
            NodeId::new(0),
            NodeCategory::FunctionalCentroid,
            "functional_centroid",
            "abstract",
            "Authentication",
        )
        .with_node_level(NodeLevel::High)
        .with_semantic_feature("Handles user login and session management");

        graph.add_node(centroid);
        graph
    }

    #[test]
    fn test_expander_creation() {
        let mut graph = create_test_graph_with_centroid();
        let expander = CentroidExpander::new(&mut graph);
        assert_eq!(expander.config.max_depth, 3);
    }

    #[test]
    fn test_infer_units_handler() {
        let mut graph = RpgGraph::new();
        let expander = CentroidExpander::new(&mut graph);

        let units = expander.infer_units_from_semantic("Handles user authentication");
        assert!(!units.is_empty());
        assert!(units.iter().any(|u| u.name == "handler"));
    }

    #[test]
    fn test_infer_units_validator() {
        let mut graph = RpgGraph::new();
        let expander = CentroidExpander::new(&mut graph);

        let units = expander.infer_units_from_semantic("Validate user input");
        assert!(!units.is_empty());
        assert!(units.iter().any(|u| u.name == "validate"));
    }

    #[test]
    fn test_infer_units_manager() {
        let mut graph = RpgGraph::new();
        let expander = CentroidExpander::new(&mut graph);

        let units = expander.infer_units_from_semantic("Manage user sessions");
        assert!(!units.is_empty());
        assert!(units.iter().any(|u| u.name == "Manager"));
    }

    #[test]
    fn test_expand_centroid() {
        let mut graph = create_test_graph_with_centroid();
        let mut expander = CentroidExpander::new(&mut graph);

        let centroid = FunctionalCentroid {
            name: "Auth".to_string(),
            description: "Authentication".to_string(),
            semantic_feature: "Handles login and logout".to_string(),
            parent: None,
        };

        let result = expander.expand_centroid(&centroid);
        assert!(result.is_ok());

        let nodes = result.expect("Should have created nodes");
        assert!(!nodes.is_empty());
    }

    #[test]
    fn test_expand_all() {
        let mut graph = create_test_graph_with_centroid();
        let mut expander = CentroidExpander::new(&mut graph);

        let result = expander.expand_all();
        assert!(result.is_ok());

        let expansion = result.expect("Should have expanded");
        assert!(expansion.centroids_expanded > 0);
        assert!(expansion.nodes_created > 0);
    }

    #[test]
    fn test_expansion_result_default() {
        let result = ExpansionResult::default();
        assert_eq!(result.nodes_created, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_config_default() {
        let config = ExpansionConfig::default();
        assert_eq!(config.max_depth, 3);
        assert_eq!(config.max_nodes_per_centroid, 10);
        assert!(config.create_categories);
    }
}


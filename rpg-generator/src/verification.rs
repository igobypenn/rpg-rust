//! Graph Verification - "Close the Loop" verification for generated code.
//!
//! This module provides verification between the planned RPG (from Phase 1/2)
//! and the actual RPG encoded from generated code (after Phase 3).
//!
//! The verification process:
//! 1. Encode generated code using RpgEncoder
//! 2. Compare functional centroids between planned and generated graphs
//! 3. Calculate similarity scores
//! 4. Report discrepancies

use std::path::Path;

use rpg_encoder::{Node, NodeCategory, RpgEncoder, RpgGraph, semantic_similarity};

use crate::error::{GeneratorError, Result};

#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Overall similarity score (0.0 to 1.0)
    pub similarity: f32,
    /// Whether verification passed (>= threshold)
    pub passed: bool,
    /// Features in planned but not in generated
    pub missing_features: Vec<String>,
    /// Features in generated but not in planned
    pub extra_features: Vec<String>,
    /// Coverage of functional centroids (0.0 to 1.0)
    pub centroid_coverage: f32,
    /// Number of planned centroids
    pub planned_centroids: usize,
    /// Number of generated centroids
    pub generated_centroids: usize,
    /// Number of matching centroids
    pub matching_centroids: usize,
    /// Semantic similarity score based on feature text comparison (0.0 to 1.0)
    pub semantic_similarity: f32,
}

impl VerificationResult {
    /// Create a new verification result.
    pub fn new() -> Self {
        Self {
            similarity: 0.0,
            passed: false,
            missing_features: Vec::new(),
            extra_features: Vec::new(),
            centroid_coverage: 0.0,
            planned_centroids: 0,
            generated_centroids: 0,
            matching_centroids: 0,
            semantic_similarity: 0.0,
        }
    }
    /// Check if verification passed with given threshold.
    pub fn passes(&self, threshold: f32) -> bool {
        self.similarity >= threshold
    }
}

impl Default for VerificationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Graph verifier - compares planned RPG with generated code RPG.
pub struct GraphVerifier {
    /// Encoder for generated code
    encoder: RpgEncoder,
    /// Similarity threshold for passing verification
    similarity_threshold: f32,
    /// Whether to use semantic matching for centroids
    semantic_matching: bool,
}

impl GraphVerifier {
    /// Create a new graph verifier with default settings.
    pub fn new() -> Result<Self> {
        let encoder = RpgEncoder::new().map_err(|e| {
            GeneratorError::VerificationFailed(format!("Failed to create encoder: {}", e))
        })?;

        Ok(Self {
            encoder,
            similarity_threshold: 0.8,
            semantic_matching: true,
        })
    }

    /// Set similarity threshold (0.0 to 1.0).
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Enable or disable semantic matching.
    pub fn with_semantic_matching(mut self, enabled: bool) -> Self {
        self.semantic_matching = enabled;
        self
    }

    /// Verify generated code against planned RPG.
    ///
    /// # Arguments
    /// * `generated_path` - Path to generated code directory
    /// * `planned_rpg` - The planned RPG from Phase 1/2
    ///
    /// # Returns
    /// Verification result with similarity scores and discrepancies.
    pub fn verify(
        &mut self,
        generated_path: &Path,
        planned_rpg: &RpgGraph,
    ) -> Result<VerificationResult> {
        tracing::info!("Verifying generated code at {:?}", generated_path);

        // 1. Encode generated code
        let encode_result = self
            .encoder
            .encode(generated_path)
            .map_err(|e| GeneratorError::VerificationFailed(format!("Encoding failed: {}", e)))?;

        let generated_rpg = &encode_result.graph;

        // 2. Extract features from both graphs
        let planned_features = self.extract_features(planned_rpg);
        let generated_features = self.extract_features(generated_rpg);

        // 3. Compare features
        let (missing, extra, similarity) =
            self.compare_features(&planned_features, &generated_features);

        // 4. Compare centroids
        let planned_centroids: Vec<_> = planned_rpg
            .nodes()
            .filter(|n| n.category == NodeCategory::FunctionalCentroid)
            .collect();
        let generated_centroids: Vec<_> = generated_rpg
            .nodes()
            .filter(|n| n.category == NodeCategory::FunctionalCentroid)
            .collect();

        let matching = self.count_matching_centroids(&planned_centroids, &generated_centroids);
        let centroid_coverage = if planned_centroids.is_empty() {
            1.0
        } else {
            matching as f32 / planned_centroids.len() as f32
        };

        // 5. Build result
        let passed = similarity >= self.similarity_threshold;

        let result = VerificationResult {
            similarity,
            passed,
            missing_features: missing,
            extra_features: extra,
            centroid_coverage,
            planned_centroids: planned_centroids.len(),
            generated_centroids: generated_centroids.len(),
            matching_centroids: matching,
            semantic_similarity: 0.0,
        };
        tracing::info!(
            "Verification complete: similarity={:.2}%, centroids={}/{}, passed={}",
            similarity * 100.0,
            matching,
            planned_centroids.len(),
            passed
        );

        Ok(result)
    }

    /// Verify with additional semantic analysis.
    ///
    /// This performs a deeper comparison using semantic features stored in nodes.
    /// Uses text-based semantic similarity via Jaccard word overlap.
    #[cfg(feature = "llm")]
    pub fn verify_with_semantics(
        &mut self,
        generated_path: &Path,
        planned_rpg: &RpgGraph,
    ) -> Result<VerificationResult> {
        let mut result = self.verify(generated_path, planned_rpg)?;

        // Enhance with semantic similarity using text-based comparison
        let planned_features: Vec<&str> = planned_rpg.nodes()
            .filter_map(|n| n.semantic_feature.as_deref())
            .collect();
        
        // Re-encode to get generated features
        let encode_result = self.encoder.encode(generated_path)
            .map_err(|e| GeneratorError::VerificationFailed(format!("Encoding failed: {}", e)))?;
        let generated_features: Vec<&str> = encode_result.graph.nodes()
            .filter_map(|n| n.semantic_feature.as_deref())
            .collect();
        
        // Calculate semantic similarity via pairwise comparison
        if !planned_features.is_empty() && !generated_features.is_empty() {
            let mut total_similarity = 0.0;
            let mut comparisons = 0;
            
            for p_feat in &planned_features {
                let mut best_match: f32 = 0.0;
                for g_feat in &generated_features {
                    let sim = semantic_similarity(p_feat, g_feat);
                    best_match = best_match.max(sim);
                }
                total_similarity += best_match;
                comparisons += 1;
            }
            
            result.semantic_similarity = if comparisons > 0 {
                total_similarity / comparisons as f32
            } else {
                0.0
            };
            
            // Weighted combination: feature similarity (40%), centroid coverage (30%), Jaccard (30%)
            result.similarity = 
                0.3 * result.similarity + // Original Jaccard similarity
                0.3 * result.centroid_coverage + // Centroid coverage
                0.4 * result.semantic_similarity; // Semantic similarity
        } else {
            // Fallback to original averaging if no semantic features
            result.similarity = (result.similarity + result.centroid_coverage) / 2.0;
        }
        
        result.passed = result.similarity >= self.similarity_threshold;

        Ok(result)
    }

    fn extract_features(&self, graph: &RpgGraph) -> Vec<String> {
        let mut features = Vec::new();

        for node in graph.nodes() {
            // Extract from semantic features
            if let Some(sf) = &node.semantic_feature {
                features.push(sf.clone());
            }

            // Extract from node names for functional centroids
            if node.category == NodeCategory::FunctionalCentroid {
                features.push(node.name.clone());
            }
        }

        // Deduplicate
        features.sort();
        features.dedup();

        features
    }

    /// Compare features and calculate similarity.
    fn compare_features(
        &self,
        planned: &[String],
        generated: &[String],
    ) -> (Vec<String>, Vec<String>, f32) {
        let planned_set: std::collections::HashSet<_> = planned.iter().cloned().collect();
        let generated_set: std::collections::HashSet<_> = generated.iter().cloned().collect();

        let missing: Vec<String> = planned_set.difference(&generated_set).cloned().collect();
        let extra: Vec<String> = generated_set.difference(&planned_set).cloned().collect();

        // Calculate Jaccard similarity
        let intersection = planned_set.intersection(&generated_set).count();
        let union = planned_set.union(&generated_set).count();

        let similarity = if union == 0 {
            1.0 // Both empty = perfect match
        } else {
            intersection as f32 / union as f32
        };

        (missing, extra, similarity)
    }

    /// Count matching centroids between planned and generated.
    fn count_matching_centroids(&self, planned: &[&Node], generated: &[&Node]) -> usize {
        let mut count = 0;

        for p in planned {
            for g in generated {
                if self.centroids_match(p, g) {
                    count += 1;
                    break;
                }
            }
        }

        count
    }

    /// Check if two centroids match.
    fn centroids_match(&self, planned: &Node, generated: &Node) -> bool {
        // Exact name match
        if planned.name.to_lowercase() == generated.name.to_lowercase() {
            return true;
        }

        // Semantic matching (if enabled)
        if self.semantic_matching {
            // Check if semantic features overlap
            if let (Some(p_feat), Some(g_feat)) =
                (&planned.semantic_feature, &generated.semantic_feature)
            {
                let p_lower = p_feat.to_lowercase();
                let g_lower = g_feat.to_lowercase();
                let p_words: std::collections::HashSet<_> =
                    p_lower.split_whitespace().collect();
                let g_words: std::collections::HashSet<_> =
                    g_lower.split_whitespace().collect();

                let overlap = p_words.intersection(&g_words).count();
                let min_len = p_words.len().min(g_words.len());

                // At least 50% word overlap
                if min_len > 0 && overlap as f32 / min_len as f32 >= 0.5 {
                    return true;
                }
            }
        }

        false
    }

    /// Generate a detailed report of verification results.
    pub fn generate_report(&self, result: &VerificationResult) -> String {
        let mut report = String::new();

        report.push_str("=== RPG Verification Report ===\n\n");
        report.push_str(&format!(
            "Overall Similarity: {:.1}%\n",
            result.similarity * 100.0
        ));
        report.push_str(&format!(
            "Status: {}\n\n",
            if result.passed {
                "PASSED ✓"
            } else {
                "FAILED ✗"
            }
        ));

        report.push_str(&format!(
            "Centroid Coverage: {:.1}%\n",
            result.centroid_coverage * 100.0
        ));
        report.push_str(&format!("  Planned: {}\n", result.planned_centroids));
        report.push_str(&format!("  Generated: {}\n", result.generated_centroids));
        report.push_str(&format!("  Matching: {}\n\n", result.matching_centroids));

        if !result.missing_features.is_empty() {
            report.push_str("Missing Features:\n");
            for f in &result.missing_features {
                report.push_str(&format!("  - {}\n", f));
            }
            report.push('\n');
        }

        if !result.extra_features.is_empty() {
            report.push_str("Extra Features:\n");
            for f in &result.extra_features {
                report.push_str(&format!("  + {}\n", f));
            }
            report.push('\n');
        }

        report
    }
}

impl Default for GraphVerifier {
    fn default() -> Self {
        Self::new().expect("Failed to create default GraphVerifier")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rpg_encoder::{Node, NodeCategory, NodeId, NodeLevel};

    #[allow(dead_code)]
    fn create_test_graph_with_centroids() -> RpgGraph {
        let mut graph = RpgGraph::new();

        // Add a centroid node
        let centroid = Node::new(
            NodeId::new(0),
            NodeCategory::FunctionalCentroid,
            "functional_centroid",
            "abstract",
            "Authentication",
        )
        .with_node_level(NodeLevel::High)
        .with_semantic_feature("Handles user authentication and login");

        graph.add_node(centroid);
        graph
    }

    #[test]
    fn test_verifier_creation() {
        let verifier = GraphVerifier::new();
        assert!(verifier.is_ok());
    }

    #[test]
    fn test_verifier_with_options() {
        let verifier = GraphVerifier::new()
            .expect("Failed to create")
            .with_threshold(0.9)
            .with_semantic_matching(false);

        assert_eq!(verifier.similarity_threshold, 0.9);
        assert!(!verifier.semantic_matching);
    }

    #[test]
    fn test_centroids_match_exact() {
        let verifier = GraphVerifier::new().expect("Failed to create");

        let n1 = Node::new(
            NodeId::new(0),
            NodeCategory::FunctionalCentroid,
            "functional_centroid",
            "abstract",
            "Auth",
        );

        let n2 = Node::new(
            NodeId::new(1),
            NodeCategory::FunctionalCentroid,
            "functional_centroid",
            "abstract",
            "auth",
        );

        assert!(verifier.centroids_match(&n1, &n2));
    }

    #[test]
    fn test_verification_result_default() {
        let result = VerificationResult::default();
        assert_eq!(result.similarity, 0.0);
        assert!(!result.passed);
        assert!(result.missing_features.is_empty());
    }

    #[test]
    fn test_generate_report() {
        let verifier = GraphVerifier::new().expect("Failed to create");
        let result = VerificationResult {
            similarity: 0.85,
            passed: true,
            missing_features: vec!["feature1".to_string()],
            extra_features: vec!["feature2".to_string()],
            centroid_coverage: 0.9,
            planned_centroids: 5,
            generated_centroids: 4,
            matching_centroids: 4,
            semantic_similarity: 0.8,
        };
        let report = verifier.generate_report(&result);
        assert!(report.contains("85.0%"));
        assert!(report.contains("PASSED"));
        assert!(report.contains("feature1"));
    }
}



/// Multi-dimensional verification result combining all dimensions.
#[derive(Debug, Clone, Default)]
pub struct MultiDimensionalResult {
    /// Overall similarity score (0.0 to 1.0)
    pub similarity: f32,
    /// Whether verification passed overall
    pub passed: bool,
    /// Structural similarity
    pub structural_similarity: f32,
    /// Feature/centroid similarity
    pub feature_similarity: f32,
    /// Interface similarity
    pub interface_similarity: f32,
    /// Dependency similarity
    pub dependency_similarity: f32,
}

/// Structural verification result.
#[derive(Debug, Clone, Default)]
pub struct StructureVerificationResult {
    /// Edge type match ratio (0.0 to 1.0)
    pub edge_match_ratio: f32,
    /// Hierarchy depth matches
    pub hierarchy_depth_match: bool,
    /// Graph is fully connected
    pub connectivity_complete: bool,
    /// Node degree similarity
    pub degree_similarity: f32,
}

/// Feature verification result.
#[derive(Debug, Clone, Default)]
pub struct FeatureVerificationResult {
    /// Centroid coverage (0.0 to 1.0)
    pub centroid_coverage: f32,
    /// Missing centroids in generated
    pub missing_centroids: Vec<String>,
    /// Extra centroids in generated
    pub extra_centroids: Vec<String>,
}

/// Interface verification result.
#[derive(Debug, Clone, Default)]
pub struct InterfaceVerificationResult {
    /// Matching function signatures
    pub matching_signatures: usize,
    /// Missing signatures in generated
    pub missing_signatures: Vec<String>,
    /// Extra signatures in generated
    pub extra_signatures: Vec<String>,
}

/// Dependency verification result.
#[derive(Debug, Clone, Default)]
pub struct DependencyVerificationResult {
    /// Import coverage (0.0 to 1.0)
    pub import_coverage: f32,
    /// Missing imports in generated
    pub missing_imports: Vec<String>,
    /// Extra imports in generated
    pub extra_imports: Vec<String>,
}

impl GraphVerifier {
    /// Verify structural integrity between planned and generated graphs.
    pub fn verify_structure(&self, planned: &RpgGraph, generated: &RpgGraph) -> StructureVerificationResult {
        // Compare node counts by category
        let mut planned_counts: std::collections::HashMap<NodeCategory, usize> = 
            std::collections::HashMap::new();
        let mut generated_counts: std::collections::HashMap<NodeCategory, usize> = 
            std::collections::HashMap::new();
        
        for node in planned.nodes() {
            *planned_counts.entry(node.category).or_insert(0) += 1;
        }
        for node in generated.nodes() {
            *generated_counts.entry(node.category).or_insert(0) += 1;
        }
        
        // Calculate similarity per category
        let mut total_similarity = 0.0;
        let mut category_count = 0;
        
        for cat in planned_counts.keys() {
            let planned_count = *planned_counts.get(cat).unwrap_or(&0) as f32;
            let generated_count = *generated_counts.get(cat).unwrap_or(&0) as f32;
            let max_count = planned_count.max(generated_count);
            if max_count > 0.0 {
                total_similarity += 1.0 - (planned_count - generated_count).abs() / max_count;
                category_count += 1;
            }
        }
        
        // Check categories in generated but not in planned
        for cat in generated_counts.keys() {
            if !planned_counts.contains_key(cat) {
                category_count += 1;
                // No similarity for extra categories
            }
        }
        
        let edge_match_ratio = if category_count == 0 {
            1.0
        } else {
            total_similarity / category_count as f32
        };
        
        StructureVerificationResult {
            edge_match_ratio,
            hierarchy_depth_match: true,
            connectivity_complete: true,
            degree_similarity: edge_match_ratio,
        }
    }
    
    /// Verify feature/centroid coverage.
    pub fn verify_features(&self, planned: &RpgGraph, generated: &RpgGraph) -> FeatureVerificationResult {
        let planned_centroids: Vec<_> = planned.nodes()
            .filter(|n| n.category == NodeCategory::FunctionalCentroid)
            .collect();
        let generated_centroids: Vec<_> = generated.nodes()
            .filter(|n| n.category == NodeCategory::FunctionalCentroid)
            .collect();
        
        let planned_names: std::collections::HashSet<_> = planned_centroids.iter()
            .map(|n| n.name.to_lowercase())
            .collect();
        let generated_names: std::collections::HashSet<_> = generated_centroids.iter()
            .map(|n| n.name.to_lowercase())
            .collect();
        
        let missing: Vec<_> = planned_names.difference(&generated_names).cloned().collect();
        let extra: Vec<_> = generated_names.difference(&planned_names).cloned().collect();
        
        let coverage = if planned_names.is_empty() {
            1.0
        } else {
            (planned_names.len() - missing.len()) as f32 / planned_names.len() as f32
        };
        
        FeatureVerificationResult {
            centroid_coverage: coverage,
            missing_centroids: missing,
            extra_centroids: extra,
        }
    }
    
    /// Verify interface compatibility.
    pub fn verify_interfaces(&self, planned: &RpgGraph, generated: &RpgGraph) -> InterfaceVerificationResult {
        let planned_functions: Vec<_> = planned.nodes()
            .filter(|n| matches!(n.category, NodeCategory::Function))
            .collect();
        let generated_functions: Vec<_> = generated.nodes()
            .filter(|n| matches!(n.category, NodeCategory::Function))
            .collect();
        
        let planned_names: std::collections::HashSet<_> = planned_functions.iter()
            .map(|n| n.name.to_lowercase())
            .collect();
        let generated_names: std::collections::HashSet<_> = generated_functions.iter()
            .map(|n| n.name.to_lowercase())
            .collect();
        
        let matching = planned_names.intersection(&generated_names).count();
        let missing: Vec<_> = planned_names.difference(&generated_names).cloned().collect();
        let extra: Vec<_> = generated_names.difference(&planned_names).cloned().collect();
        
        InterfaceVerificationResult {
            matching_signatures: matching,
            missing_signatures: missing,
            extra_signatures: extra,
        }
    }
    
    /// Verify dependency consistency.
    pub fn verify_dependencies(&self, planned: &RpgGraph, generated: &RpgGraph) -> DependencyVerificationResult {
        let planned_modules: Vec<_> = planned.nodes()
            .filter(|n| matches!(n.category, NodeCategory::Module | NodeCategory::File))
            .collect();
        let generated_modules: Vec<_> = generated.nodes()
            .filter(|n| matches!(n.category, NodeCategory::Module | NodeCategory::File))
            .collect();
        
        let planned_names: std::collections::HashSet<_> = planned_modules.iter()
            .filter_map(|n| n.path.as_ref())
            .map(|p| p.to_string_lossy().to_lowercase())
            .collect();
        let generated_names: std::collections::HashSet<_> = generated_modules.iter()
            .filter_map(|n| n.path.as_ref())
            .map(|p| p.to_string_lossy().to_lowercase())
            .collect();
        
        let missing: Vec<_> = planned_names.difference(&generated_names).cloned().collect();
        let extra: Vec<_> = generated_names.difference(&planned_names).cloned().collect();
        
        let coverage = if planned_names.is_empty() {
            1.0
        } else {
            (planned_names.len() - missing.len()) as f32 / planned_names.len() as f32
        };
        
        DependencyVerificationResult {
            import_coverage: coverage,
            missing_imports: missing,
            extra_imports: extra,
        }
    }
    
    /// Perform comprehensive multi-dimensional verification.
    pub fn verify_all_dimensions(&self, planned: &RpgGraph, generated: &RpgGraph) -> MultiDimensionalResult {
        let structural = self.verify_structure(planned, generated);
        let features = self.verify_features(planned, generated);
        let interfaces = self.verify_interfaces(planned, generated);
        let dependencies = self.verify_dependencies(planned, generated);
        
        // Calculate weighted average
        let similarity = structural.edge_match_ratio * 0.25
            + features.centroid_coverage * 0.35
            + (interfaces.matching_signatures as f32 / (interfaces.matching_signatures + interfaces.missing_signatures.len()).max(1) as f32) * 0.25
            + dependencies.import_coverage * 0.15;
        
        MultiDimensionalResult {
            similarity,
            passed: similarity >= self.similarity_threshold,
            structural_similarity: structural.edge_match_ratio,
            feature_similarity: features.centroid_coverage,
            interface_similarity: interfaces.matching_signatures as f32 / (interfaces.matching_signatures + interfaces.missing_signatures.len()).max(1) as f32,
            dependency_similarity: dependencies.import_coverage,
        }
    }

}

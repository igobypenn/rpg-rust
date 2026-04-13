use std::path::{Path, PathBuf};

use super::diff::{self, FileDiff, ModifiedFile};
use super::hash::compute_hash;
use super::snapshot::{CachedUnit, RpgSnapshot, UnitType};
use crate::core::{EdgeType, Node, NodeCategory, NodeId};
use crate::error::Result;
use crate::parser::{LanguageParser, ParserRegistry};

#[cfg(feature = "llm")]
use crate::agents::{FeatureExtractor, SemanticConfig};
#[cfg(feature = "llm")]
use crate::error::RpgError;

#[derive(Debug, Clone, Default)]
pub struct EvolutionSummary {
    pub files_added: usize,
    pub files_deleted: usize,
    pub files_modified: usize,
    pub units_added: usize,
    pub units_changed: usize,
    pub units_deleted: usize,
    pub nodes_created: usize,
    pub nodes_removed: usize,
    pub nodes_updated: usize,
    pub edges_rebuilt: usize,
    pub llm_calls: usize,
    pub cache_hits: usize,
    /// V^H centroids invalidated due to V^L changes
    pub centroids_invalidated: usize,
    /// V^H centroids re-computed
    pub centroids_recomputed: usize,
    /// BelongsToFeature edges invalidated due to V^L changes
    pub feature_edges_invalidated: usize,
    /// BelongsToFeature edges re-linked after centroid updates
    pub feature_edges_relinked: usize,
}
pub struct RpgEvolution<'a> {
    snapshot: &'a mut RpgSnapshot,
    registry: &'a ParserRegistry,
}

fn create_file_node(file_path: &Path, language: &str) -> Node {
    Node::new(
        NodeId::new(0),
        NodeCategory::File,
        "file",
        language,
        file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown"),
    )
    .with_path(file_path.to_path_buf())
}

impl<'a> RpgEvolution<'a> {
    pub fn new(snapshot: &'a mut RpgSnapshot, registry: &'a ParserRegistry) -> Self {
        Self { snapshot, registry }
    }

    pub async fn process_diff(
        &mut self,
        diff: FileDiff,
        #[cfg(feature = "llm")] config: Option<&SemanticConfig>,
    ) -> Result<EvolutionSummary> {
        let mut summary = EvolutionSummary {
            files_deleted: diff.deleted.len(),
            files_added: diff.added.len(),
            files_modified: diff.modified.len(),
            ..Default::default()
        };

        let deleted_result = self.process_deleted_files(&diff.deleted)?;
        summary.nodes_removed = deleted_result.nodes_removed.len();
        summary.edges_rebuilt += deleted_result.edges_removed;

        #[cfg(feature = "llm")]
        let added_result = self.process_added_files(&diff.added, config).await?;
        #[cfg(not(feature = "llm"))]
        let added_result = self.process_added_files(&diff.added).await?;

        summary.nodes_created = added_result.nodes_created.len();
        #[cfg(feature = "llm")]
        {
            summary.llm_calls += added_result.llm_calls;
        }

        #[cfg(feature = "llm")]
        let modified_result = self.process_modified_files(&diff.modified, config).await?;
        #[cfg(not(feature = "llm"))]
        let modified_result = self.process_modified_files(&diff.modified).await?;

        summary.units_added += modified_result.units_added;
        summary.units_changed += modified_result.units_changed;
        summary.units_deleted += modified_result.units_deleted;
        summary.nodes_created += modified_result.nodes_created.len();
        summary.nodes_removed += modified_result.nodes_removed;
        summary.cache_hits += modified_result.cache_hits;
        #[cfg(feature = "llm")]
        {
            summary.llm_calls += modified_result.llm_calls;
        }

        // === V^H Centroid Invalidation ===
        // Collect all changed nodes for centroid invalidation
        let mut all_changed_nodes = Vec::new();
        all_changed_nodes.extend(deleted_result.nodes_removed.clone());
        all_changed_nodes.extend(added_result.nodes_created.clone());
        all_changed_nodes.extend(modified_result.nodes_created.clone());

        // Invalidate stale BelongsToFeature edges from changed V^L nodes
        summary.feature_edges_invalidated = self.invalidate_stale_feature_edges(&all_changed_nodes);

        // Invalidate affected V^H centroids
        summary.centroids_invalidated = self.invalidate_stale_centroids(&all_changed_nodes);

        // Re-compute invalidated centroids
        #[cfg(feature = "llm")]
        {
            summary.centroids_recomputed = self.recompute_centroids().await?;
        }

        // Re-link V^L nodes to appropriate V^H centroids
        summary.feature_edges_relinked = self.relink_feature_edges(&all_changed_nodes);

        self.snapshot.build_reverse_deps();
        self.snapshot.compute_file_hashes()?;
        self.snapshot.update_timestamp();

        Ok(summary)
    }
    fn process_deleted_files(&mut self, files: &[PathBuf]) -> Result<DeleteResult> {
        let mut result = DeleteResult::default();

        for file_path in files {
            let nodes = self.snapshot.graph.remove_file_nodes(file_path);
            result.nodes_removed.extend(nodes);

            self.snapshot.unit_cache.remove(file_path);
            self.snapshot.file_hashes.remove(file_path);

            for &node_id in &result.nodes_removed {
                let dependents = self.snapshot.dependents_of(node_id);
                for dep_id in dependents {
                    if let Some(node) = self.snapshot.graph.get_node_mut(dep_id) {
                        node.features.clear();
                        node.description = None;
                    }
                }
            }
        }

        result.edges_removed = self.cleanup_orphaned_edges();
        Ok(result)
    }

    async fn process_added_files(
        &mut self,
        files: &[PathBuf],
        #[cfg(feature = "llm")] config: Option<&SemanticConfig>,
    ) -> Result<AddResult> {
        let mut result = AddResult::default();

        for file_path in files {
            let full_path = self.snapshot.repo_dir.join(file_path);

            if !full_path.exists() {
                continue;
            }

            let parser = match self.registry.get_parser(&full_path) {
                Some(p) => p,
                None => continue,
            };

            let source = std::fs::read_to_string(&full_path)?;
            let file_hash = compute_hash(&source);
            self.snapshot
                .file_hashes
                .insert(file_path.clone(), file_hash);

            #[cfg(feature = "llm")]
            {
                if let Some(config) = config {
                    match self
                        .process_file_with_llm(file_path, &source, parser, config)
                        .await
                    {
                        Ok(units) => {
                            result.llm_calls += 1;
                            for unit in &units {
                                if let Some(node_id) = unit.node_id {
                                    result.nodes_created.push(node_id);
                                }
                            }
                            self.snapshot.insert_units(file_path.clone(), units);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "LLM processing failed for {}: {}",
                                file_path.display(),
                                e
                            );
                        }
                    }
                    continue;
                }
            }

            let units = self.process_file_structural(file_path, &source, parser)?;
            for unit in &units {
                if let Some(node_id) = unit.node_id {
                    result.nodes_created.push(node_id);
                }
            }
            self.snapshot.insert_units(file_path.clone(), units);
        }

        Ok(result)
    }

    fn process_file_structural(
        &mut self,
        file_path: &Path,
        source: &str,
        parser: &dyn LanguageParser,
    ) -> Result<Vec<CachedUnit>> {
        let parse_result = parser.parse(source, file_path)?;
        let mut units = Vec::new();

        let file_node = create_file_node(file_path, parser.language_name());
        let file_node_id = self.snapshot.graph.add_node(file_node);

        for def in &parse_result.definitions {
            let Some(unit_type) = UnitType::from_kind(def.kind.as_str()) else {
                continue;
            };

            let (start_line, end_line) = def
                .location
                .as_ref()
                .map(|l| (l.start_line, l.end_line))
                .unwrap_or((1, 1));

            let content = diff::extract_unit_content(source, start_line, end_line);
            let content_hash = compute_hash(&content);

            let node = Node::new(
                NodeId::new(0),
                if unit_type == UnitType::Function {
                    NodeCategory::Function
                } else {
                    NodeCategory::Type
                },
                def.kind.clone(),
                parser.language_name(),
                def.name.clone(),
            )
            .with_path(file_path.to_path_buf());

            let node_id = self.snapshot.graph.add_node(node);
            self.snapshot
                .graph
                .add_typed_edge(file_node_id, node_id, EdgeType::Contains);

            let cached_unit = CachedUnit::new(
                def.name.clone(),
                unit_type,
                content_hash,
                start_line,
                end_line,
            )
            .with_node_id(node_id);

            units.push(cached_unit);
        }

        Ok(units)
    }

    #[cfg(feature = "llm")]
    async fn process_file_with_llm(
        &mut self,
        file_path: &Path,
        source: &str,
        parser: &dyn LanguageParser,
        config: &SemanticConfig,
    ) -> Result<Vec<CachedUnit>> {
        let extractor = FeatureExtractor::new(config.clone()).map_err(|e| {
            RpgError::Incremental(format!("Failed to create feature extractor: {}", e))
        })?;

        let repo_info = format!("Repository: {}", self.snapshot.repo_name);
        let organized = extractor
            .extract_and_organize(source, file_path, &repo_info, "")
            .await
            .map_err(|e| RpgError::Incremental(format!("Feature extraction failed: {}", e)))?;

        let mut units = Vec::new();
        let parse_result = parser.parse(source, file_path)?;

        let file_node = create_file_node(file_path, parser.language_name());
        let file_node_id = self.snapshot.graph.add_node(file_node);

        for def in &parse_result.definitions {
            let Some(unit_type) = UnitType::from_kind(def.kind.as_str()) else {
                continue;
            };

            let (start_line, end_line) = def
                .location
                .as_ref()
                .map(|l| (l.start_line, l.end_line))
                .unwrap_or((1, 1));

            let content = diff::extract_unit_content(source, start_line, end_line);
            let content_hash = compute_hash(&content);

            let org_feature = organized.iter().find(|o| o.entity_name == def.name);

            let node = Node::new(
                NodeId::new(0),
                if unit_type == UnitType::Function {
                    NodeCategory::Function
                } else {
                    NodeCategory::Type
                },
                def.kind.clone(),
                parser.language_name(),
                def.name.clone(),
            )
            .with_path(file_path.to_path_buf())
            .with_features(org_feature.map(|o| o.features.clone()).unwrap_or_default())
            .with_description(
                org_feature
                    .map(|o| o.description.clone())
                    .unwrap_or_default(),
            )
            .with_feature_path(
                org_feature
                    .map(|o| o.feature_path.clone())
                    .unwrap_or_default(),
            );

            let node_id = self.snapshot.graph.add_node(node);
            self.snapshot
                .graph
                .add_typed_edge(file_node_id, node_id, EdgeType::Contains);

            let cached_unit = CachedUnit::new(
                def.name.clone(),
                unit_type,
                content_hash,
                start_line,
                end_line,
            )
            .with_features(org_feature.map(|o| o.features.clone()).unwrap_or_default())
            .with_description(
                org_feature
                    .map(|o| o.description.clone())
                    .unwrap_or_default(),
            )
            .with_node_id(node_id);

            units.push(cached_unit);
        }

        Ok(units)
    }

    async fn process_modified_files(
        &mut self,
        files: &[ModifiedFile],
        #[cfg(feature = "llm")] config: Option<&SemanticConfig>,
    ) -> Result<UpdateResult> {
        let mut result = UpdateResult::default();

        for modified in files {
            self.process_deleted_files(std::slice::from_ref(&modified.path))?;

            let full_path = self.snapshot.repo_dir.join(&modified.path);

            if !full_path.exists() {
                continue;
            }

            let parser = match self.registry.get_parser(&full_path) {
                Some(p) => p,
                None => continue,
            };

            let source = std::fs::read_to_string(&full_path)?;

            #[cfg(feature = "llm")]
            {
                if let Some(config) = config {
                    let units = self
                        .process_file_with_llm(&modified.path, &source, parser, config)
                        .await?;
                    result.llm_calls += 1;
                    result.units_added += modified.added_units.len();
                    result.units_changed += modified.changed_units.len();
                    result.units_deleted += modified.deleted_units.len();

                    for unit in &units {
                        if let Some(node_id) = unit.node_id {
                            result.nodes_created.push(node_id);
                        }
                    }
                    self.snapshot
                        .unit_cache
                        .insert(modified.path.clone(), units);
                } else {
                    result.cache_hits += modified.unchanged_units.len();
                }
            }

            #[cfg(not(feature = "llm"))]
            {
                let units = self.process_file_structural(&modified.path, &source, parser)?;
                result.units_added += modified.added_units.len();
                result.units_changed += modified.changed_units.len();
                result.units_deleted += modified.deleted_units.len();
                result.cache_hits += modified.unchanged_units.len();

                for unit in &units {
                    if let Some(node_id) = unit.node_id {
                        result.nodes_created.push(node_id);
                    }
                }
                self.snapshot
                    .unit_cache
                    .insert(modified.path.clone(), units);
            }

            self.snapshot
                .file_hashes
                .insert(modified.path.clone(), modified.new_hash.clone());
        }

        Ok(result)
    }

    fn cleanup_orphaned_edges(&mut self) -> usize {
        let mut orphaned = 0;
        let valid_nodes: std::collections::HashSet<NodeId> = self
            .snapshot
            .graph
            .nodes()
            .filter(|n| !n.name.is_empty())
            .map(|n| n.id)
            .collect();

        self.snapshot.graph.retain_edges(|s, t, _| {
            let valid = valid_nodes.contains(&s) && valid_nodes.contains(&t);
            if !valid {
                orphaned += 1;
            }
            valid
        });

        orphaned
    }

    /// Invalidate V^H centroids whose member V^L nodes have changed.
    ///
    /// This is critical for maintaining graph consistency during incremental
    /// evolution. When V^L nodes are modified/deleted, their parent V^H centroids
    /// may need re-computation.
    ///
    /// Returns the number of centroids invalidated.
    pub fn invalidate_stale_centroids(&mut self, changed_nodes: &[NodeId]) -> usize {
        use crate::core::NodeLevel;

        // Find V^H centroids that contain any of the changed V^L nodes
        let mut invalidated = std::collections::HashSet::new();

        for node_id in changed_nodes {
            // Walk up the graph to find parent centroids (edges_to returns incoming edges)
            for (source_id, _edge) in self.snapshot.graph.edges_to(*node_id) {
                let source = match self.snapshot.graph.get_node(source_id) {
                    Some(n) => n,
                    None => continue,
                };
                if source.node_level == NodeLevel::High {
                    invalidated.insert(source_id);
                }
            }
        }

        let count = invalidated.len();

        // Mark centroids as needing re-computation by clearing their semantic features
        for centroid_id in &invalidated {
            if let Some(node) = self.snapshot.graph.get_node_mut(*centroid_id) {
                // Clear semantic feature to indicate staleness
                node.semantic_feature = None;
            }
        }

        tracing::info!(
            "Invalidated {} V^H centroids due to {} changed V^L nodes",
            count,
            changed_nodes.len()
        );

        count
    }

    /// Re-compute invalidated V^H centroids.
    ///
    /// This should be called after invalidation to update centroid metadata
    /// based on their current V^L members.
    #[cfg(feature = "llm")]
    pub async fn recompute_centroids(&mut self) -> Result<usize> {
        use crate::core::NodeLevel;

        // Find stale centroids (semantic_feature is None)
        let stale_centroids: Vec<NodeId> = self
            .snapshot
            .graph
            .nodes()
            .filter(|n| n.node_level == NodeLevel::High && n.semantic_feature.is_none())
            .map(|n| n.id)
            .collect();

        let count = stale_centroids.len();

        if count == 0 {
            return Ok(0);
        }

        // First pass: collect features for each centroid (without holding references)
        let mut centroid_features: std::collections::HashMap<NodeId, Vec<String>> =
            std::collections::HashMap::new();

        for centroid_id in &stale_centroids {
            let mut features = Vec::new();

            for node in self.snapshot.graph.nodes() {
                if node.node_level == NodeLevel::Low {
                    // Check if this node is connected to the centroid
                    if self
                        .snapshot
                        .graph
                        .edge_between(node.id, *centroid_id)
                        .is_some()
                    {
                        if let Some(feat) = &node.semantic_feature {
                            features.push(feat.clone());
                        }
                    }
                }
            }

            centroid_features.insert(*centroid_id, features);
        }

        // Second pass: update centroids with collected features
        for (centroid_id, features) in centroid_features {
            if let Some(centroid) = self.snapshot.graph.get_node_mut(centroid_id) {
                centroid.semantic_feature = if features.is_empty() {
                    Some("unknown".to_string())
                } else {
                    Some(features.join("; "))
                };
            }
        }

        tracing::info!("Re-computed {} V^H centroids", count);
        Ok(count)
    }

    /// Invalidate BelongsToFeature edges from changed V^L nodes.
    ///
    /// Per paper Section 3.2: When V^L nodes are modified, their edges to V^H centroids
    /// may become stale if the node's semantic feature changed significantly.
    ///
    /// This method removes BelongsToFeature edges from changed nodes so they can
    /// be re-linked to appropriate centroids during re-computation.
    ///
    /// Returns the number of edges removed.
    pub fn invalidate_stale_feature_edges(&mut self, changed_nodes: &[NodeId]) -> usize {
        use crate::core::NodeLevel;

        let changed_set: std::collections::HashSet<NodeId> =
            changed_nodes.iter().copied().collect();
        let mut removed = 0;

        // Collect edges to remove first (can't mutate while iterating)
        let edges_to_remove: Vec<(NodeId, NodeId)> = self
            .snapshot
            .graph
            .edges()
            .filter_map(|(source, target, edge)| {
                // Only BelongsToFeature edges from changed V^L nodes to V^H centroids
                if edge.edge_type == EdgeType::BelongsToFeature && changed_set.contains(&source) {
                    // Verify target is a V^H centroid
                    if let Some(target_node) = self.snapshot.graph.get_node(target) {
                        if target_node.node_level == NodeLevel::High {
                            return Some((source, target));
                        }
                    }
                }
                None
            })
            .collect();

        // Remove the collected edges
        for (source, target) in &edges_to_remove {
            self.snapshot.graph.remove_edge_between(*source, *target);
            removed += 1;
        }

        if removed > 0 {
            tracing::info!(
                "Invalidated {} BelongsToFeature edges from {} changed V^L nodes",
                removed,
                changed_nodes.len()
            );
        }

        removed
    }

    /// Re-link V^L nodes to V^H centroids based on semantic feature matching.
    ///
    /// Per paper Section 3.3: After V^H centroids are re-computed, V^L nodes should
    /// be linked to their best-matching centroid based on semantic similarity.
    ///
    /// This method finds the best centroid match for each node's semantic feature
    /// and creates a new BelongsToFeature edge.
    ///
    /// Returns the number of edges created.
    pub fn relink_feature_edges(&mut self, changed_nodes: &[NodeId]) -> usize {
        use crate::core::NodeLevel;

        let mut linked = 0;

        // Get all V^H centroids with semantic features
        let centroids: Vec<(NodeId, String)> = self
            .snapshot
            .graph
            .nodes()
            .filter(|n| n.node_level == NodeLevel::High)
            .filter_map(|n| n.semantic_feature.as_ref().map(|sf| (n.id, sf.clone())))
            .collect();

        if centroids.is_empty() {
            return 0;
        }

        // Re-link each changed V^L node to its best-matching centroid
        for &node_id in changed_nodes {
            let node = match self.snapshot.graph.get_node(node_id) {
                Some(n) => n,
                None => continue,
            };

            // Skip nodes that aren't V^L or don't have semantic features
            if node.node_level != NodeLevel::Low {
                continue;
            }

            let node_feature = match &node.semantic_feature {
                Some(f) => f,
                None => continue,
            };

            // Find best matching centroid using word overlap (same as functional.rs)
            let best_match = self.find_best_centroid_match(node_feature, &centroids);

            if let Some(centroid_id) = best_match {
                // Check if edge already exists
                if self
                    .snapshot
                    .graph
                    .edge_between(node_id, centroid_id)
                    .is_none()
                {
                    self.snapshot.graph.add_typed_edge(
                        node_id,
                        centroid_id,
                        EdgeType::BelongsToFeature,
                    );
                    linked += 1;
                }
            }
        }

        if linked > 0 {
            tracing::info!(
                "Re-linked {} BelongsToFeature edges for {} changed V^L nodes",
                linked,
                changed_nodes.len()
            );
        }

        linked
    }

    /// Find the best matching centroid for a semantic feature.
    ///
    /// Uses word overlap scoring similar to FunctionalAbstraction::find_best_centroid().
    fn find_best_centroid_match(
        &self,
        feature: &str,
        centroids: &[(NodeId, String)],
    ) -> Option<NodeId> {
        let feature_lower = feature.to_lowercase();
        let mut best_match: Option<(NodeId, usize)> = None;

        for (centroid_id, centroid_feature) in centroids {
            let centroid_lower = centroid_feature.to_lowercase();

            // Score based on word overlap
            let score = if feature_lower.contains(&centroid_lower)
                || centroid_lower
                    .split_whitespace()
                    .any(|w| feature_lower.contains(w))
            {
                centroid_lower.split_whitespace().count()
            } else {
                0
            };

            if score > 0 {
                match &best_match {
                    None => best_match = Some((*centroid_id, score)),
                    Some((_, best_score)) if score > *best_score => {
                        best_match = Some((*centroid_id, score));
                    }
                    _ => {}
                }
            }
        }

        best_match.map(|(id, _)| id)
    }
}
#[derive(Debug, Clone, Default)]
struct DeleteResult {
    nodes_removed: Vec<NodeId>,
    edges_removed: usize,
}

#[derive(Debug, Clone, Default)]
struct AddResult {
    nodes_created: Vec<NodeId>,
    #[cfg(feature = "llm")]
    llm_calls: usize,
}

#[derive(Debug, Clone, Default)]
struct UpdateResult {
    units_added: usize,
    units_changed: usize,
    units_deleted: usize,
    nodes_created: Vec<NodeId>,
    nodes_removed: usize,
    #[cfg(feature = "llm")]
    llm_calls: usize,
    cache_hits: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{EdgeType, Node, NodeCategory, NodeId, NodeLevel};

    fn make_snapshot() -> RpgSnapshot {
        RpgSnapshot::new("test-repo", Path::new("/tmp/test-evolution"))
    }

    fn make_registry() -> ParserRegistry {
        ParserRegistry::new()
    }

    #[test]
    fn test_evolution_new() {
        let mut snapshot = make_snapshot();
        let registry = make_registry();
        let _evo = RpgEvolution::new(&mut snapshot, &registry);
    }

    #[test]
    fn test_evolution_summary_default() {
        let s = EvolutionSummary::default();
        assert_eq!(s.files_added, 0);
        assert_eq!(s.files_deleted, 0);
        assert_eq!(s.files_modified, 0);
        assert_eq!(s.units_added, 0);
        assert_eq!(s.units_changed, 0);
        assert_eq!(s.units_deleted, 0);
        assert_eq!(s.nodes_created, 0);
        assert_eq!(s.nodes_removed, 0);
        assert_eq!(s.nodes_updated, 0);
        assert_eq!(s.edges_rebuilt, 0);
        assert_eq!(s.llm_calls, 0);
        assert_eq!(s.cache_hits, 0);
        assert_eq!(s.centroids_invalidated, 0);
        assert_eq!(s.centroids_recomputed, 0);
        assert_eq!(s.feature_edges_invalidated, 0);
        assert_eq!(s.feature_edges_relinked, 0);
    }

    #[test]
    fn test_invalidate_stale_centroids_no_changes() {
        let mut snapshot = make_snapshot();
        let registry = make_registry();
        let mut evo = RpgEvolution::new(&mut snapshot, &registry);

        let count = evo.invalidate_stale_centroids(&[]);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_invalidate_stale_centroids_with_vl_node() {
        let mut snapshot = make_snapshot();
        let registry = make_registry();

        let centroid_id = snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::FunctionalCentroid,
                "functional_centroid",
                "abstract",
                "Auth",
            )
            .with_node_level(NodeLevel::High)
            .with_semantic_feature("authentication"),
        );

        let vl_id = snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::Function,
                "fn",
                "rust",
                "login",
            )
            .with_node_level(NodeLevel::Low),
        );

        snapshot
            .graph
            .add_typed_edge(centroid_id, vl_id, EdgeType::Contains);

        let mut evo = RpgEvolution::new(&mut snapshot, &registry);
        let count = evo.invalidate_stale_centroids(&[vl_id]);
        assert_eq!(count, 1);

        let centroid = snapshot
            .graph
            .get_node(centroid_id)
            .expect("centroid exists");
        assert!(centroid.semantic_feature.is_none());
    }

    #[test]
    fn test_invalidate_stale_feature_edges() {
        let mut snapshot = make_snapshot();
        let registry = make_registry();

        let centroid_id = snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::FunctionalCentroid,
                "functional_centroid",
                "abstract",
                "Auth",
            )
            .with_node_level(NodeLevel::High),
        );

        let vl_id = snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::Function,
                "fn",
                "rust",
                "login",
            )
            .with_node_level(NodeLevel::Low),
        );

        snapshot
            .graph
            .add_typed_edge(vl_id, centroid_id, EdgeType::BelongsToFeature);
        assert_eq!(snapshot.graph.edge_count(), 1);

        let mut evo = RpgEvolution::new(&mut snapshot, &registry);
        let removed = evo.invalidate_stale_feature_edges(&[vl_id]);
        assert_eq!(removed, 1);
        assert_eq!(snapshot.graph.edge_count(), 0);
    }

    #[test]
    fn test_relink_feature_edges() {
        let mut snapshot = make_snapshot();
        let registry = make_registry();

        let _centroid_id = snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::FunctionalCentroid,
                "functional_centroid",
                "abstract",
                "Auth",
            )
            .with_node_level(NodeLevel::High)
            .with_semantic_feature("authentication"),
        );

        let vl_id = snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::Function,
                "fn",
                "rust",
                "login",
            )
            .with_node_level(NodeLevel::Low)
            .with_semantic_feature("authentication login"),
        );

        let mut evo = RpgEvolution::new(&mut snapshot, &registry);
        let linked = evo.relink_feature_edges(&[vl_id]);
        assert_eq!(linked, 1);
        assert_eq!(snapshot.graph.edge_count(), 1);
    }

    #[test]
    fn test_process_deleted_files() {
        let mut snapshot = make_snapshot();
        let registry = make_registry();

        let file_path = PathBuf::from("src/deleted.rs");
        let file_node = snapshot.graph.add_node(
            Node::new(
                NodeId::new(0),
                NodeCategory::File,
                "file",
                "rust",
                "deleted.rs",
            )
            .with_path(file_path.clone()),
        );

        snapshot.build_reverse_deps();

        let mut evo = RpgEvolution::new(&mut snapshot, &registry);
        let result = evo.process_deleted_files(&[file_path]).expect("ok");

        assert!(result.nodes_removed.contains(&file_node));
        assert_eq!(snapshot.graph.node_count(), 0);
    }

    #[test]
    fn test_evolution_summary_clone_debug() {
        let mut s = EvolutionSummary::default();
        s.files_added = 1;
        s.centroids_invalidated = 2;

        let cloned = s.clone();
        assert_eq!(cloned.files_added, 1);
        assert_eq!(cloned.centroids_invalidated, 2);
        assert!(format!("{:?}", s).contains("files_added"));
    }
}

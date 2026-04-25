use std::path::Path;

use super::diff;
use super::evolution::{create_file_node, RpgEvolution};
use super::hash::compute_hash;
use super::snapshot::{CachedUnit, UnitType};

use crate::agents::{FeatureExtractor, SemanticConfig};
use crate::core::{EdgeType, Node, NodeCategory, NodeId};
use crate::error::{Result, RpgError};
use crate::parser::LanguageParser;

impl<'a> RpgEvolution<'a> {
    pub(crate) async fn process_file_with_llm(
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

    pub async fn recompute_centroids(&mut self) -> Result<usize> {
        use crate::core::NodeLevel;

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

        let mut centroid_features: std::collections::HashMap<NodeId, Vec<String>> =
            std::collections::HashMap::new();

        for centroid_id in &stale_centroids {
            let mut features = Vec::new();

            for node in self.snapshot.graph.nodes() {
                if node.node_level == NodeLevel::Low {
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
}

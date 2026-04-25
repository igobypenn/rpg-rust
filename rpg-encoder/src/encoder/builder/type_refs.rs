use super::GraphBuilder;
use crate::core::{Edge, EdgeType};

impl GraphBuilder {
    pub fn link_type_refs(mut self) -> Self {
        let unresolved = std::mem::take(&mut self.unresolved_type_refs);

        for (_source_id, type_ref, source_file) in unresolved {
            let same_file = self
                .qualified_defs
                .get(&(source_file.clone(), type_ref.type_name.clone()));

            let import_match = self.find_via_imports(&source_file, &type_ref.type_name);

            let bare_match = self
                .bare_name_defs
                .get(&type_ref.type_name)
                .and_then(|entries| {
                    if entries.len() == 1 {
                        Some(entries[0].1)
                    } else {
                        None
                    }
                });

            let type_id = same_file.copied().or(import_match).or(bare_match);

            if let Some(type_id) = type_id {
                let mut edge = Edge::new(EdgeType::UsesType);
                edge.metadata.insert(
                    "ref_kind".to_string(),
                    serde_json::Value::String(format!("{:?}", type_ref.ref_kind).to_lowercase()),
                );
                self.graph.add_edge(_source_id, type_id, edge);
            }
        }

        self
    }
}

use std::collections::HashMap;

use super::GraphBuilder;
use crate::core::{Edge, EdgeType, NodeCategory};

impl GraphBuilder {
    pub fn link_extends(mut self) -> Self {
        let type_defs: HashMap<String, crate::core::NodeId> = self
            .graph
            .nodes()
            .filter(|n| n.category == NodeCategory::Type)
            .map(|n| (n.name.clone(), n.id))
            .collect();

        let mut extends_to_add: Vec<(crate::core::NodeId, crate::core::NodeId)> = Vec::new();

        for node in self.graph.nodes() {
            let bases = match node.metadata.get("bases") {
                Some(b) => b,
                None => continue,
            };
            let arr = match bases.as_array() {
                Some(a) => a,
                None => continue,
            };
            for base in arr {
                if let Some(base_name) = base.as_str() {
                    if let Some(&base_id) = type_defs.get(base_name) {
                        extends_to_add.push((node.id, base_id));
                    }
                }
            }
        }

        for (derived_id, base_id) in extends_to_add {
            self.graph
                .add_edge(derived_id, base_id, Edge::new(EdgeType::Extends));
        }

        self
    }
}

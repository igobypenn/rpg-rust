use std::collections::HashMap;

use crate::core::{Edge, EdgeType};
use super::GraphBuilder;

impl GraphBuilder {
    pub fn link_impls(mut self) -> Self {
        let traits: HashMap<String, crate::core::NodeId> = self.graph.nodes()
            .filter(|n| n.kind == "trait")
            .map(|n| (n.name.clone(), n.id))
            .collect();

        for (impl_id, trait_name) in &self.unresolved_impls {
            if let Some(&trait_id) = traits.get(trait_name) {
                self.graph.add_edge(*impl_id, trait_id, Edge::new(EdgeType::Implements));
            }
        }
        self
    }
}

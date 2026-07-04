use super::GraphBuilder;
use crate::core::{Edge, EdgeType, NodeCategory, NodeId};

impl GraphBuilder {
    pub fn link_imports(mut self) -> Self {
        let edges_to_add: Vec<(NodeId, NodeId)> = self
            .graph
            .nodes()
            .filter(|n| n.category == NodeCategory::Import)
            .filter_map(|node| {
                // Last ::-segment (e.g. `std::io::Read` -> `Read`), no alloc.
                if let Some(last) = node.name.rsplit("::").next() {
                    if let Some(entries) = self.bare_name_defs.get(last) {
                        if let Some((_, def_id)) = entries.first() {
                            return Some((node.id, *def_id));
                        }
                    }
                }
                None
            })
            .collect();

        for (src, tgt) in edges_to_add {
            self.graph
                .add_edge(src, tgt, Edge::new(EdgeType::References));
        }

        self
    }
}

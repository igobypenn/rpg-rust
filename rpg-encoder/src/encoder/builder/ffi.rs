use crate::core::{Edge, EdgeType, Node, NodeCategory, NodeId};
use super::GraphBuilder;

impl GraphBuilder {
    pub fn link_ffi(mut self) -> Self {
        for (file_id, binding) in &self.ffi_bindings {
            let binding_node = Node::new(
                NodeId::new(self.graph.node_count()),
                NodeCategory::Feature,
                "ffi_binding",
                &binding.source_lang,
                &binding.symbol,
            );
            let binding_id = self.graph.add_node(binding_node);
            let mut edge = Edge::new(EdgeType::FfiBinding);
            for (k, v) in binding.to_metadata() {
                edge.metadata.insert(k, v);
            }
            self.graph.add_edge(*file_id, binding_id, edge);
        }
        self
    }
}

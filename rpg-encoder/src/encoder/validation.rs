use crate::core::{EdgeType, NodeCategory, RpgGraph};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub total_nodes: usize,
    pub total_edges: usize,
    pub edge_type_counts: HashMap<String, usize>,
    pub node_category_counts: HashMap<String, usize>,
    pub import_resolution_rate: f64,
    pub call_edge_count: usize,
    pub implements_edge_count: usize,
    pub ffi_edge_count: usize,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn from_graph(graph: &RpgGraph) -> Self {
        let mut edge_type_counts = HashMap::new();
        let mut node_category_counts = HashMap::new();
        let mut import_count = 0usize;
        let mut resolved_imports = 0usize;

        for node in graph.nodes() {
            let cat = format!("{:?}", node.category).to_lowercase();
            *node_category_counts.entry(cat).or_insert(0) += 1;
            if node.category == NodeCategory::Import {
                import_count += 1;
            }
        }

        for (src, _tgt, edge) in graph.edges() {
            let et = format!("{:?}", edge.edge_type).to_lowercase();
            *edge_type_counts.entry(et.clone()).or_insert(0) += 1;
            if (edge.edge_type == EdgeType::References || edge.edge_type == EdgeType::Imports)
                && graph
                    .get_node(src)
                    .is_some_and(|n| n.category == NodeCategory::Import)
            {
                resolved_imports += 1;
            }
        }

        let import_resolution_rate = if import_count > 0 {
            resolved_imports as f64 / import_count as f64
        } else {
            1.0
        };

        let mut warnings = Vec::new();
        if import_resolution_rate < 0.5 && import_count > 10 {
            warnings.push(format!(
                "Low import resolution: {:.1}% ({} of {})",
                import_resolution_rate * 100.0,
                resolved_imports,
                import_count
            ));
        }

        let call_edge_count = edge_type_counts.get("calls").copied().unwrap_or(0);
        let implements_edge_count = edge_type_counts.get("implements").copied().unwrap_or(0);
        let ffi_edge_count = edge_type_counts.get("ffi_binding").copied().unwrap_or(0);

        Self {
            total_nodes: graph.node_count(),
            total_edges: graph.edge_count(),
            edge_type_counts,
            node_category_counts,
            import_resolution_rate,
            call_edge_count,
            implements_edge_count,
            ffi_edge_count,
            warnings,
        }
    }
}

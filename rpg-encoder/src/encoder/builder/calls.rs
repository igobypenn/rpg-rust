use crate::core::{Edge, EdgeType, NodeCategory, NodeId};
use super::GraphBuilder;
use std::path::Path;

impl GraphBuilder {
    pub fn link_calls(mut self) -> Self {
        let unresolved = std::mem::take(&mut self.unresolved_calls);

        for (caller_id, call, caller_file) in unresolved {
            if crate::languages::builtins::is_common_method_call(&call.callee) {
                continue;
            }

            let mut resolved = false;

            if let Some(ref receiver) = call.receiver {
                if !receiver.is_empty() {
                    let receiver_file = self.graph.nodes()
                        .filter(|n| n.name == *receiver && n.category == NodeCategory::Type)
                        .find_map(|n| n.path.clone());

                    if let Some(target_file) = receiver_file {
                        let target_id = self.qualified_defs
                            .get(&(target_file.clone(), call.callee.clone()))
                            .copied()
                            .or_else(|| {
                                self.bare_name_defs.get(&call.callee)
                                    .and_then(|entries| {
                                        entries.iter()
                                            .find(|(file, _)| *file == target_file)
                                            .map(|(_, id)| *id)
                                    })
                            });

                        if let Some(target_id) = target_id {
                            let mut edge = Edge::new(EdgeType::Calls);
                            edge.metadata.insert(
                                "receiver".to_string(),
                                serde_json::Value::String(receiver.clone()),
                            );
                            edge.metadata.insert(
                                "call_kind".to_string(),
                                serde_json::Value::String(format!("{:?}", call.call_kind).to_lowercase()),
                            );
                            self.graph.add_edge(caller_id, target_id, edge);
                            resolved = true;
                        }
                    }
                }
            }

            if !resolved {
                let same_file = self
                    .qualified_defs
                    .get(&(caller_file.clone(), call.callee.clone()));

                let import_match = self.find_via_imports(&caller_file, &call.callee);

                let bare_match = self.bare_name_defs.get(&call.callee).and_then(|entries| {
                    if entries.len() == 1 {
                        Some(entries[0].1)
                    } else {
                        None
                    }
                });

                let callee_id = same_file.copied().or(import_match).or(bare_match);

                if let Some(callee_id) = callee_id {
                    let mut edge = Edge::new(EdgeType::Calls);
                    if let Some(ref receiver) = call.receiver {
                        edge.metadata.insert(
                            "receiver".to_string(),
                            serde_json::Value::String(receiver.clone()),
                        );
                    }
                    edge.metadata.insert(
                        "call_kind".to_string(),
                        serde_json::Value::String(format!("{:?}", call.call_kind).to_lowercase()),
                    );
                    self.graph.add_edge(caller_id, callee_id, edge);
                }
            }
        }

        self
    }

    pub(crate) fn find_via_imports(&self, file_path: &Path, callee: &str) -> Option<NodeId> {
        let imports = self.file_imports.get(file_path)?;
        for import in imports {
            let parts: Vec<&str> = import.module_path.split("::").collect();
            if let Some(imported_name) = parts.last() {
                if imported_name == &callee
                    || import.imported_names.iter().any(|n| n == callee)
                {
                    if let Some(entries) = self.bare_name_defs.get(callee) {
                        return Some(entries.first()?.1);
                    }
                }
            }
        }
        None
    }
}

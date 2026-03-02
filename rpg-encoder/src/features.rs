//! Feature tree and node types for property-level planning.

use serde::{Deserialize, Serialize};

/// Hierarchical feature taxonomy representing what a repository does.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureTree {
    pub root: FeatureNode,
    pub version: String,
}

impl FeatureTree {
    pub fn new(root_name: &str) -> Self {
        Self {
            root: FeatureNode::new(root_name),
            version: "1.0".to_string(),
        }
    }

    pub fn all_features(&self) -> Vec<&str> {
        let mut features = Vec::new();
        self.collect_features(&self.root, &mut features);
        features
    }

    fn collect_features<'a>(&'a self, node: &'a FeatureNode, features: &mut Vec<&'a str>) {
        features.extend(node.features.iter().map(|s| s.as_str()));
        for child in &node.children {
            self.collect_features(child, features);
        }
    }

    pub fn find_node(&self, name: &str) -> Option<&FeatureNode> {
        self.find_node_recursive(&self.root, name)
    }

    fn find_node_recursive<'a>(
        &'a self,
        node: &'a FeatureNode,
        name: &str,
    ) -> Option<&'a FeatureNode> {
        if node.name == name {
            return Some(node);
        }
        for child in &node.children {
            if let Some(found) = self.find_node_recursive(child, name) {
                return Some(found);
            }
        }
        None
    }

    pub fn to_flat(&self) -> Vec<FlatFeature> {
        let mut flat = Vec::new();
        self.flatten_recursive(&self.root, Vec::new(), &mut flat);
        flat
    }

    fn flatten_recursive(
        &self,
        node: &FeatureNode,
        path: Vec<String>,
        flat: &mut Vec<FlatFeature>,
    ) {
        let mut current_path = path;
        current_path.push(node.name.clone());

        for feature in &node.features {
            flat.push(FlatFeature {
                path: current_path.clone(),
                feature: feature.clone(),
            });
        }

        for child in &node.children {
            self.flatten_recursive(child, current_path.clone(), flat);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureNode {
    pub name: String,
    pub description: Option<String>,
    pub children: Vec<FeatureNode>,
    pub features: Vec<String>,
}

impl FeatureNode {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            description: None,
            children: Vec::new(),
            features: Vec::new(),
        }
    }

    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    pub fn add_child(&mut self, child: FeatureNode) {
        self.children.push(child);
    }

    pub fn add_feature(&mut self, feature: &str) {
        self.features.push(feature.to_string());
    }

    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    pub fn depth(&self) -> usize {
        if self.children.is_empty() {
            return 1;
        }
        1 + self.children.iter().map(|c| c.depth()).max().unwrap_or(0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatFeature {
    pub path: Vec<String>,
    pub feature: String,
}

impl FlatFeature {
    pub fn full_path(&self) -> String {
        format!("{}::{}", self.path.join("::"), self.feature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_tree() {
        let mut tree = FeatureTree::new("root");
        let mut gameplay = FeatureNode::new("gameplay");
        gameplay.add_feature("movement");
        gameplay.add_feature("collision");
        tree.root.add_child(gameplay);

        assert_eq!(tree.all_features(), vec!["movement", "collision"]);
        assert!(tree.find_node("gameplay").is_some());
    }

    #[test]
    fn test_to_flat() {
        let mut tree = FeatureTree::new("root");
        let mut gameplay = FeatureNode::new("gameplay");
        gameplay.add_feature("movement");
        tree.root.add_child(gameplay);

        let flat = tree.to_flat();
        assert_eq!(flat.len(), 1);
        assert_eq!(flat[0].path, vec!["root", "gameplay"]);
        assert_eq!(flat[0].feature, "movement");
    }

    #[test]
    fn test_serialization() {
        let mut tree = FeatureTree::new("root");
        let mut node = FeatureNode::new("test");
        node.add_feature("feature1");
        tree.root.add_child(node);

        let json = serde_json::to_string(&tree).unwrap();
        let deserialized: FeatureTree = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.all_features(), tree.all_features());
    }
}

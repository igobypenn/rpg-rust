#[cfg(feature = "opencode")]
use super::types::*;
#[cfg(feature = "opencode")]
use crate::error::Result;
#[cfg(feature = "opencode")]
use crate::types::{ArchitectureDesign, GenerationPlan};
#[cfg(feature = "opencode")]
use std::path::PathBuf;

#[cfg(feature = "opencode")]
pub fn parse_phase1_response(
    json_str: &str,
    description: &str,
) -> Result<(rpg_encoder::FeatureTree, rpg_encoder::ComponentPlan)> {
    if let Ok(combined) = serde_json::from_str::<serde_json::Value>(json_str) {
        let feature_tree = if let Some(categories) = combined.get("categories") {
            json_to_feature_tree(categories, description)
        } else if let Ok(response) = serde_json::from_str::<FeatureExtractionResponse>(json_str) {
            response_to_feature_tree(response)
        } else {
            rpg_encoder::FeatureTree::new("project")
        };

        let component_plan = if let Some(components) = combined.get("components") {
            json_to_component_plan(components)
        } else {
            infer_components_from_features(&feature_tree)
        };

        return Ok((feature_tree, component_plan));
    }

    if let Ok(response) = serde_json::from_str::<FeatureExtractionResponse>(json_str) {
        let feature_tree = response_to_feature_tree(response);
        let component_plan = infer_components_from_features(&feature_tree);
        return Ok((feature_tree, component_plan));
    }

    tracing::warn!("Failed to parse agent response, creating basic plan");
    let feature_tree = rpg_encoder::FeatureTree::new("project");
    let component_plan = rpg_encoder::ComponentPlan::new(vec![]);
    Ok((feature_tree, component_plan))
}

#[cfg(feature = "opencode")]
fn response_to_feature_tree(response: FeatureExtractionResponse) -> rpg_encoder::FeatureTree {
    let mut tree = rpg_encoder::FeatureTree::new(&response.root_name);

    for category in response.categories {
        let mut category_node = rpg_encoder::FeatureNode::new(&category.name);

        if let Some(desc) = &category.description {
            category_node = category_node.with_description(desc);
        }

        for feature in &category.features {
            category_node.add_feature(feature);
        }

        for subcategory in &category.subcategories {
            let mut sub_node = rpg_encoder::FeatureNode::new(&subcategory.name);
            for feature in &subcategory.features {
                sub_node.add_feature(feature);
            }
            category_node.add_child(sub_node);
        }

        tree.root.add_child(category_node);
    }

    tree
}

#[cfg(feature = "opencode")]
fn json_to_feature_tree(
    value: &serde_json::Value,
    description: &str,
) -> rpg_encoder::FeatureTree {
    let mut tree = rpg_encoder::FeatureTree::new("project");

    if let Some(categories) = value.as_array() {
        for cat in categories {
            if let Some(name) = cat.get("name").and_then(|n| n.as_str()) {
                let mut category_node = rpg_encoder::FeatureNode::new(name);

                if let Some(desc) = cat.get("description").and_then(|d| d.as_str()) {
                    category_node = category_node.with_description(desc);
                }

                if let Some(features) = cat.get("features").and_then(|f| f.as_array()) {
                    for feature in features {
                        if let Some(feat_name) = feature.as_str() {
                            category_node.add_feature(feat_name);
                        }
                    }
                }

                tree.root.add_child(category_node);
            }
        }
    }

    if tree.root.children.is_empty() {
        let mut feature_node = rpg_encoder::FeatureNode::new("main");
        feature_node.add_feature(description);
        tree.root.add_child(feature_node);
    }

    tree
}

#[cfg(feature = "opencode")]
fn json_to_component_plan(value: &serde_json::Value) -> rpg_encoder::ComponentPlan {
    let mut components = Vec::new();

    if let Some(comps) = value.as_array() {
        for comp in comps {
            if let Some(name) = comp.get("name").and_then(|n| n.as_str()) {
                let description = comp
                    .get("description")
                    .and_then(|d| d.as_str())
                    .unwrap_or_default()
                    .to_string();

                let mut component = rpg_encoder::Component::new(name, &description);

                if let Some(features) = comp.get("features").and_then(|f| f.as_array()) {
                    for feature in features {
                        if let Some(feat_name) = feature.as_str() {
                            component.subtree.add_feature(feat_name);
                        }
                    }
                }

                components.push(component);
            }
        }
    }

    rpg_encoder::ComponentPlan::new(components)
}

#[cfg(feature = "opencode")]
fn infer_components_from_features(
    feature_tree: &rpg_encoder::FeatureTree,
) -> rpg_encoder::ComponentPlan {
    let mut components = Vec::new();

    for child in &feature_tree.root.children {
        let mut component = rpg_encoder::Component::new(
            &child.name,
            &child.description.clone().unwrap_or_default(),
        );

        for feature in &child.features {
            component.subtree.add_feature(feature);
        }

        for subchild in &child.children {
            for feature in &subchild.features {
                component.subtree.add_feature(feature);
            }
        }

        components.push(component);
    }

    if components.is_empty() {
        let mut core = rpg_encoder::Component::new("core", "Core functionality");
        for feature in feature_tree.all_features() {
            core.subtree.add_feature(feature);
        }
        components.push(core);
    }

    rpg_encoder::ComponentPlan::new(components)
}

#[cfg(feature = "opencode")]
pub fn parse_phase2_response(
    output: &crate::agent::AgentOutput,
    plan: &GenerationPlan,
) -> Result<ArchitectureDesign> {
    let json_str = output
        .as_json()
        .map(|v| v.to_string())
        .unwrap_or_else(|| output.to_text());

    if let Ok(response) = serde_json::from_str::<SkeletonResponse>(&json_str) {
        return Ok(skeleton_response_to_design(&response, plan));
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&json_str) {
        return Ok(json_to_design(&value, plan));
    }

    tracing::warn!("Failed to parse architecture response, creating basic design");
    Ok(create_default_design(plan))
}

#[cfg(feature = "opencode")]
fn skeleton_response_to_design(
    response: &SkeletonResponse,
    plan: &GenerationPlan,
) -> ArchitectureDesign {
    let mut skeleton = rpg_encoder::RepoSkeleton::new(PathBuf::from("src"), "rust");
    let mut task_plan = rpg_encoder::TaskPlan::new();

    for file_design in &response.files {
        let mut file = rpg_encoder::SkeletonFile::new(PathBuf::from(&file_design.path), "rust");

        if let Some(units) = &file_design.units {
            for unit in units {
                let kind = unit.kind.parse::<rpg_encoder::UnitKind>()
                    .unwrap_or(rpg_encoder::UnitKind::Function);

                let mut unit_skeleton = rpg_encoder::UnitSkeleton::new(&unit.name, kind)
                    .with_features(unit.features.clone());

                if let Some(sig) = &unit.signature {
                    unit_skeleton = unit_skeleton.with_signature(sig);
                }
                if let Some(doc) = &unit.docstring {
                    unit_skeleton = unit_skeleton.with_docstring(doc);
                }

                file.add_unit(unit_skeleton);
            }
        }

        skeleton.add_file(file);
    }

    for component in &plan.component_plan.components {
        let task = rpg_encoder::ImplementationTask::new(
            &format!("task_{}", component.name.replace('.', "_")),
            PathBuf::from(format!("src/{}/mod.rs", component.name.replace('.', "/"))),
            &component.name,
        );
        task_plan.add_batch(&component.name, vec![task]);
    }

    ArchitectureDesign::new(uuid::Uuid::new_v4(), skeleton, task_plan)
}

#[cfg(feature = "opencode")]
fn json_to_design(value: &serde_json::Value, plan: &GenerationPlan) -> ArchitectureDesign {
    let mut skeleton = rpg_encoder::RepoSkeleton::new(PathBuf::from("src"), "rust");
    let mut task_plan = rpg_encoder::TaskPlan::new();

    if let Some(files) = value.get("files").and_then(|f| f.as_array()) {
        for file_val in files {
            if let Some(path) = file_val.get("path").and_then(|p| p.as_str()) {
                let file = rpg_encoder::SkeletonFile::new(PathBuf::from(path), "rust");
                skeleton.add_file(file);
            }
        }
    }

    for component in &plan.component_plan.components {
        let task = rpg_encoder::ImplementationTask::new(
            &format!("task_{}", component.name.replace('.', "_")),
            PathBuf::from(format!("src/{}/mod.rs", component.name.replace('.', "/"))),
            &component.name,
        );
        task_plan.add_batch(&component.name, vec![task]);
    }

    ArchitectureDesign::new(uuid::Uuid::new_v4(), skeleton, task_plan)
}

#[cfg(feature = "opencode")]
fn create_default_design(plan: &GenerationPlan) -> ArchitectureDesign {
    let mut skeleton = rpg_encoder::RepoSkeleton::new(PathBuf::from("src"), "rust");
    let mut task_plan = rpg_encoder::TaskPlan::new();

    for component in &plan.component_plan.components {
        let file = rpg_encoder::SkeletonFile::new(
            PathBuf::from(format!("src/{}/mod.rs", component.name.replace('.', "/"))),
            "rust",
        );
        skeleton.add_file(file);

        let task = rpg_encoder::ImplementationTask::new(
            &format!("task_{}", component.name.replace('.', "_")),
            PathBuf::from(format!("src/{}/mod.rs", component.name.replace('.', "/"))),
            &component.name,
        );
        task_plan.add_batch(&component.name, vec![task]);
    }

    ArchitectureDesign::new(uuid::Uuid::new_v4(), skeleton, task_plan)
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "opencode")]
    use super::*;

    #[cfg(feature = "opencode")]
    #[test]
    fn test_parse_feature_response() {
        let json = r#"{
            "root_name": "test_project",
            "categories": [
                {
                    "name": "auth",
                    "description": "Authentication",
                    "features": ["login", "logout"],
                    "subcategories": []
                }
            ]
        }"#;

        let response: FeatureExtractionResponse = serde_json::from_str(json).expect("Failed to parse");
        let tree = response_to_feature_tree(response);

        assert_eq!(tree.root.name, "test_project");
        assert_eq!(tree.root.children.len(), 1);
        assert_eq!(tree.root.children[0].name, "auth");
    }

    #[cfg(feature = "opencode")]
    #[test]
    fn test_infer_components() {
        let mut tree = rpg_encoder::FeatureTree::new("project");
        let mut auth = rpg_encoder::FeatureNode::new("auth");
        auth.add_feature("login");
        tree.root.add_child(auth);

        let plan = infer_components_from_features(&tree);
        assert_eq!(plan.components.len(), 1);
        assert_eq!(plan.components[0].name, "auth");
    }
}

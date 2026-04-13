//! Phase 2: Implementation Level - Architecture design and task planning.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::contract::ContractVerifier;
use crate::error::Result;
use crate::llm::{LlmClient, OpenAIClient};
use crate::types::{ArchitectureDesign, FileInterface, GenerationPlan, UnitInterface};
use crate::UnitKind;
use rpg_encoder::{ImplementationTask, RepoSkeleton, SkeletonFile, TaskPlan};

pub struct ImplementationLevelBuilder {
    client: OpenAIClient,
    verifier: ContractVerifier,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SkeletonResponse {
    directories: Vec<String>,
    files: Vec<FileResponse>,
    entry_point: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileResponse {
    path: String,
    purpose: String,
    component: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InterfaceResponse {
    imports: Vec<String>,
    units: Vec<UnitResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UnitResponse {
    name: String,
    kind: String,
    signature: String,
    docstring: Option<String>,
    features: Vec<String>,
}

impl ImplementationLevelBuilder {
    pub fn new(client: OpenAIClient) -> Self {
        Self {
            client,
            verifier: ContractVerifier::new(),
        }
    }

    pub fn with_verifier(mut self, verifier: ContractVerifier) -> Self {
        self.verifier = verifier;
        self
    }

    pub async fn build(&self, plan: &GenerationPlan) -> Result<ArchitectureDesign> {
        let skeleton = self.design_skeleton(plan).await?;
        let interfaces = self.design_interfaces(&skeleton, plan).await?;
        let task_plan = self.plan_tasks(&skeleton, &interfaces, plan);

        let mut design = ArchitectureDesign::new(plan.id, skeleton, task_plan);

        for (_path, interface) in interfaces {
            design.add_interface(interface);
        }

        design.complete();

        self.verifier.verify_architecture_design(&design, plan)?;

        Ok(design)
    }

    async fn design_skeleton(&self, plan: &GenerationPlan) -> Result<RepoSkeleton> {
        let components: String = plan
            .component_plan
            .components
            .iter()
            .map(|c| format!("- {}: {}", c.name, c.description))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = super::prompts::SKELETON_DESIGN_PROMPT
            .replace("{language}", plan.request.language.as_str())
            .replace("{components}", &components);

        let response: SkeletonResponse = self
            .client
            .complete_json_raw("", &prompt)
            .await
            .and_then(|v| self.client.deserialize(v))?;
        Ok(self.response_to_skeleton(response, &plan.request.language))
    }

    fn response_to_skeleton(
        &self,
        response: SkeletonResponse,
        language: &crate::types::TargetLanguage,
    ) -> RepoSkeleton {
        let mut skeleton = RepoSkeleton::new(PathBuf::from("."), language.as_str());

        for file in response.files {
            let skeleton_file = SkeletonFile::new(PathBuf::from(&file.path), language.as_str());
            skeleton.add_file(skeleton_file);
        }

        skeleton
    }

    async fn design_interfaces(
        &self,
        skeleton: &RepoSkeleton,
        plan: &GenerationPlan,
    ) -> Result<std::collections::HashMap<PathBuf, FileInterface>> {
        let mut interfaces = std::collections::HashMap::new();

        for file in &skeleton.files {
            let component_name = self.find_component_for_file(&file.path, plan);
            let features = self.find_features_for_component(&component_name, plan);

            let interface = self
                .design_file_interface(
                    &file.path,
                    &component_name,
                    &features,
                    &plan.request.language,
                )
                .await?;

            interfaces.insert(file.path.clone(), interface);
        }

        Ok(interfaces)
    }

    async fn design_file_interface(
        &self,
        file_path: &std::path::Path,
        component: &str,
        features: &[String],
        language: &crate::types::TargetLanguage,
    ) -> Result<FileInterface> {
        let prompt = super::prompts::INTERFACE_DESIGN_PROMPT
            .replace("{language}", language.as_str())
            .replace("{file_path}", &file_path.display().to_string())
            .replace("{component}", component)
            .replace("{features}", &features.join("\n"));

        let response: InterfaceResponse = self
            .client
            .complete_json_raw("", &prompt)
            .await
            .and_then(|v| self.client.deserialize(v))?;
        Ok(self.response_to_interface(file_path.to_path_buf(), response))
    }

    fn response_to_interface(&self, path: PathBuf, response: InterfaceResponse) -> FileInterface {
        let units: Vec<UnitInterface> = response
            .units
            .into_iter()
            .map(|u| UnitInterface {
                name: u.name,
                kind: self.parse_unit_kind(&u.kind),
                signature: Some(u.signature),
                docstring: u.docstring,
                features: u.features,
            })
            .collect();

        FileInterface {
            path,
            units,
            imports: response.imports,
        }
    }

    fn parse_unit_kind(&self, kind: &str) -> UnitKind {
        match kind.to_lowercase().as_str() {
            "function" | "fn" => UnitKind::Function,
            "method" => UnitKind::Method,
            "class" => UnitKind::Class,
            "struct" => UnitKind::Struct,
            "enum" => UnitKind::Enum,
            "interface" | "trait" => UnitKind::Interface,
            "module" => UnitKind::Module,
            "constant" | "const" => UnitKind::Constant,
            _ => UnitKind::Function,
        }
    }

    fn find_component_for_file(&self, path: &std::path::Path, plan: &GenerationPlan) -> String {
        let path_str = path.display().to_string();

        for component in &plan.component_plan.components {
            let component_dir = component.name.replace('.', "/");
            if path_str.contains(&component_dir) {
                return component.name.clone();
            }
        }

        "shared".to_string()
    }

    fn find_features_for_component(
        &self,
        component_name: &str,
        plan: &GenerationPlan,
    ) -> Vec<String> {
        plan.component_plan
            .components
            .iter()
            .find(|c| c.name == component_name)
            .map(|c| c.all_features().into_iter().map(String::from).collect())
            .unwrap_or_default()
    }

    fn plan_tasks(
        &self,
        skeleton: &RepoSkeleton,
        interfaces: &std::collections::HashMap<PathBuf, FileInterface>,
        plan: &GenerationPlan,
    ) -> TaskPlan {
        let mut task_plan = TaskPlan::new();

        for component in &plan.component_plan.components {
            let files: Vec<_> = skeleton
                .files
                .iter()
                .filter(|f| {
                    let component_dir = component.name.replace('.', "/");
                    f.path.display().to_string().contains(&component_dir)
                })
                .collect();

            let tasks: Vec<ImplementationTask> = files
                .iter()
                .map(|file| {
                    let interface = interfaces.get(&file.path);
                    self.create_task_for_file(&file.path, interface, &component.name)
                })
                .collect();

            task_plan.add_batch(&component.name, tasks);
        }

        task_plan
    }

    fn create_task_for_file(
        &self,
        path: &std::path::Path,
        interface: Option<&FileInterface>,
        component: &str,
    ) -> ImplementationTask {
        let mut task = ImplementationTask::new(
            &format!(
                "task_{}",
                path.display().to_string().replace(['/', '.'], "_")
            ),
            path.to_path_buf(),
            component,
        );

        if let Some(iface) = interface {
            for unit in &iface.units {
                let features = unit.features.clone();
                let signature = unit.signature.clone().unwrap_or_default();
                task.add_unit(&unit.name, &signature, features);
            }
        }

        task
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TargetLanguage;
    use std::collections::HashMap;

    fn create_builder() -> ImplementationLevelBuilder {
        let config = crate::llm::LlmConfig::new("test-key");
        let client = OpenAIClient::new(config).unwrap();
        ImplementationLevelBuilder::new(client)
    }

    fn create_test_plan() -> GenerationPlan {
        use crate::ComponentPlan;
        use crate::FeatureTree;
        use crate::GenerationRequest;

        let mut tree = FeatureTree::new("test");
        tree.root.add_feature("feature1");

        let mut component = rpg_encoder::Component::new("gameplay.core", "Core gameplay logic");
        component.subtree.add_feature("feature1");

        GenerationPlan::new(
            GenerationRequest::new("Test project", TargetLanguage::Rust),
            tree,
            ComponentPlan::new(vec![component]),
        )
    }

    #[test]
    fn test_parse_unit_kind_function() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("function"), UnitKind::Function);
    }

    #[test]
    fn test_parse_unit_kind_fn() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("fn"), UnitKind::Function);
    }

    #[test]
    fn test_parse_unit_kind_method() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("method"), UnitKind::Method);
    }

    #[test]
    fn test_parse_unit_kind_class() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("class"), UnitKind::Class);
    }

    #[test]
    fn test_parse_unit_kind_struct() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("struct"), UnitKind::Struct);
    }

    #[test]
    fn test_parse_unit_kind_enum() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("enum"), UnitKind::Enum);
    }

    #[test]
    fn test_parse_unit_kind_interface() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("interface"), UnitKind::Interface);
    }

    #[test]
    fn test_parse_unit_kind_trait() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("trait"), UnitKind::Interface);
    }

    #[test]
    fn test_parse_unit_kind_module() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("module"), UnitKind::Module);
    }

    #[test]
    fn test_parse_unit_kind_constant() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("constant"), UnitKind::Constant);
    }

    #[test]
    fn test_parse_unit_kind_const() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("const"), UnitKind::Constant);
    }

    #[test]
    fn test_parse_unit_kind_case_insensitive() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("FUNCTION"), UnitKind::Function);
        assert_eq!(builder.parse_unit_kind("Struct"), UnitKind::Struct);
        assert_eq!(builder.parse_unit_kind("ENUM"), UnitKind::Enum);
    }

    #[test]
    fn test_parse_unit_kind_default() {
        let builder = create_builder();
        assert_eq!(builder.parse_unit_kind("unknown"), UnitKind::Function);
        assert_eq!(builder.parse_unit_kind(""), UnitKind::Function);
        assert_eq!(builder.parse_unit_kind("variable"), UnitKind::Function);
    }

    #[test]
    fn test_find_component_for_file_matches_component() {
        let builder = create_builder();
        let plan = create_test_plan();
        let path = std::path::Path::new("src/gameplay/core/mod.rs");
        assert_eq!(
            builder.find_component_for_file(path, &plan),
            "gameplay.core"
        );
    }

    #[test]
    fn test_find_component_for_file_returns_shared_when_no_match() {
        let builder = create_builder();
        let plan = create_test_plan();
        let path = std::path::Path::new("src/utils/helpers.rs");
        assert_eq!(builder.find_component_for_file(path, &plan), "shared");
    }

    #[test]
    fn test_find_features_for_component_found() {
        let builder = create_builder();
        let plan = create_test_plan();
        let features = builder.find_features_for_component("gameplay.core", &plan);
        assert_eq!(features, vec!["feature1"]);
    }

    #[test]
    fn test_find_features_for_component_not_found() {
        let builder = create_builder();
        let plan = create_test_plan();
        let features = builder.find_features_for_component("nonexistent", &plan);
        assert!(features.is_empty());
    }

    #[test]
    fn test_response_to_skeleton() {
        let builder = create_builder();
        let response = SkeletonResponse {
            directories: vec!["src".to_string()],
            files: vec![
                FileResponse {
                    path: "src/main.rs".to_string(),
                    purpose: "entry point".to_string(),
                    component: "core".to_string(),
                },
                FileResponse {
                    path: "src/lib.rs".to_string(),
                    purpose: "library root".to_string(),
                    component: "core".to_string(),
                },
            ],
            entry_point: Some("src/main.rs".to_string()),
        };
        let language = TargetLanguage::Rust;
        let skeleton = builder.response_to_skeleton(response, &language);
        assert_eq!(skeleton.files.len(), 2);
        assert_eq!(skeleton.files[0].path, PathBuf::from("src/main.rs"));
        assert_eq!(skeleton.files[1].path, PathBuf::from("src/lib.rs"));
    }

    #[test]
    fn test_response_to_interface() {
        let builder = create_builder();
        let response = InterfaceResponse {
            imports: vec!["std::collections::HashMap".to_string()],
            units: vec![
                UnitResponse {
                    name: "process".to_string(),
                    kind: "function".to_string(),
                    signature: "fn process(input: &str) -> Result<usize>".to_string(),
                    docstring: Some("Processes the input".to_string()),
                    features: vec!["feature1".to_string()],
                },
                UnitResponse {
                    name: "Handler".to_string(),
                    kind: "struct".to_string(),
                    signature: "pub struct Handler { count: usize }".to_string(),
                    docstring: None,
                    features: vec![],
                },
            ],
        };
        let path = PathBuf::from("src/main.rs");
        let interface = builder.response_to_interface(path.clone(), response);

        assert_eq!(interface.path, path);
        assert_eq!(interface.imports, vec!["std::collections::HashMap"]);
        assert_eq!(interface.units.len(), 2);
        assert_eq!(interface.units[0].name, "process");
        assert_eq!(interface.units[0].kind, UnitKind::Function);
        assert_eq!(
            interface.units[0].signature.as_deref(),
            Some("fn process(input: &str) -> Result<usize>")
        );
        assert_eq!(
            interface.units[0].docstring.as_deref(),
            Some("Processes the input")
        );
        assert_eq!(interface.units[0].features, vec!["feature1"]);
        assert_eq!(interface.units[1].name, "Handler");
        assert_eq!(interface.units[1].kind, UnitKind::Struct);
        assert!(interface.units[1].docstring.is_none());
    }

    #[test]
    fn test_plan_tasks() {
        let builder = create_builder();
        let plan = create_test_plan();

        let mut skeleton = RepoSkeleton::new(PathBuf::from("."), "rust");
        skeleton.add_file(SkeletonFile::new(
            PathBuf::from("src/gameplay/core/mod.rs"),
            "rust",
        ));
        skeleton.add_file(SkeletonFile::new(
            PathBuf::from("src/gameplay/core/player.rs"),
            "rust",
        ));

        let mut interfaces = HashMap::new();
        let file_path1 = PathBuf::from("src/gameplay/core/mod.rs");
        interfaces.insert(
            file_path1.clone(),
            FileInterface {
                path: file_path1,
                units: vec![UnitInterface {
                    name: "init".to_string(),
                    kind: UnitKind::Function,
                    signature: Some("fn init()".to_string()),
                    docstring: None,
                    features: vec!["feature1".to_string()],
                }],
                imports: vec![],
            },
        );

        let task_plan = builder.plan_tasks(&skeleton, &interfaces, &plan);
        assert!(task_plan.batches.contains_key("gameplay.core"));
        let tasks = task_plan.tasks_for_component("gameplay.core");
        assert_eq!(tasks.len(), 2);
    }

    #[test]
    fn test_create_task_for_file_with_interface() {
        let builder = create_builder();
        let path = std::path::Path::new("src/gameplay/core/mod.rs");
        let interface = FileInterface {
            path: path.to_path_buf(),
            units: vec![
                UnitInterface {
                    name: "init".to_string(),
                    kind: UnitKind::Function,
                    signature: Some("fn init() -> bool".to_string()),
                    docstring: None,
                    features: vec!["feature1".to_string()],
                },
                UnitInterface {
                    name: "update".to_string(),
                    kind: UnitKind::Method,
                    signature: None,
                    docstring: Some("Updates state".to_string()),
                    features: vec![],
                },
            ],
            imports: vec![],
        };

        let task = builder.create_task_for_file(path, Some(&interface), "gameplay.core");
        assert_eq!(task.units.len(), 2);
        assert!(task.units.contains(&"init".to_string()));
        assert!(task.units.contains(&"update".to_string()));
        assert_eq!(task.unit_code.get("init").unwrap(), "fn init() -> bool");
        assert_eq!(task.unit_code.get("update").unwrap(), "");
        assert_eq!(
            task.unit_features.get("init").unwrap(),
            &vec!["feature1".to_string()]
        );
        assert!(task.unit_features.get("update").unwrap().is_empty());
    }

    #[test]
    fn test_create_task_for_file_without_interface() {
        let builder = create_builder();
        let path = std::path::Path::new("src/utils/helpers.rs");
        let task = builder.create_task_for_file(path, None, "shared");
        assert!(task.units.is_empty());
        assert_eq!(task.subtree, "shared");
    }
}

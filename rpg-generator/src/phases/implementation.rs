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

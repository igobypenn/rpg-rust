//! File skeleton and interface types for implementation-level planning.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSkeleton {
    pub root: PathBuf,
    pub language: String,
    pub files: Vec<SkeletonFile>,
    pub version: String,
}

impl RepoSkeleton {
    pub fn new(root: PathBuf, language: &str) -> Self {
        Self {
            root,
            language: language.to_string(),
            files: Vec::new(),
            version: "1.0".to_string(),
        }
    }

    pub fn add_file(&mut self, file: SkeletonFile) {
        self.files.push(file);
    }

    pub fn files_for_component(&self, component: &str) -> Vec<&SkeletonFile> {
        self.files
            .iter()
            .filter(|f| {
                f.path
                    .to_str()
                    .map(|s| s.contains(&component.replace('.', "/")))
                    .unwrap_or(false)
            })
            .collect()
    }

    pub fn topological_order(&self) -> Vec<&SkeletonFile> {
        let mut in_degree: HashMap<PathBuf, usize> = HashMap::new();
        let mut dependents: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

        for file in &self.files {
            in_degree.entry(file.path.clone()).or_insert(0);
            for dep in &file.imports {
                if dep.starts_with("./") || dep.starts_with("../") {
                    let dep_path = self.resolve_import(&file.path, dep);
                    dependents
                        .entry(dep_path)
                        .or_default()
                        .push(file.path.clone());
                    *in_degree.entry(file.path.clone()).or_insert(0) += 1;
                }
            }
        }

        let mut queue: Vec<PathBuf> = in_degree
            .iter()
            .filter(|(_, &deg)| deg == 0)
            .map(|(p, _)| p.clone())
            .collect();

        let mut result = Vec::new();

        while let Some(path) = queue.pop() {
            if let Some(file) = self.files.iter().find(|f| f.path == path) {
                result.push(file);
            }

            if let Some(deps) = dependents.get(&path) {
                for dep in deps {
                    if let Some(deg) = in_degree.get_mut(dep) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(dep.clone());
                        }
                    }
                }
            }
        }

        if result.len() != self.files.len() {
            return self.files.iter().collect();
        }

        result
    }

    fn resolve_import(&self, from: &Path, import: &str) -> PathBuf {
        let import = import.trim_start_matches("./").trim_start_matches("../");
        from.parent()
            .map(|p| p.join(import))
            .unwrap_or_else(|| PathBuf::from(import))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkeletonFile {
    pub path: PathBuf,
    pub language: String,
    pub units: Vec<UnitSkeleton>,
    pub imports: Vec<String>,
}

impl SkeletonFile {
    pub fn new(path: PathBuf, language: &str) -> Self {
        Self {
            path,
            language: language.to_string(),
            units: Vec::new(),
            imports: Vec::new(),
        }
    }

    pub fn add_unit(&mut self, unit: UnitSkeleton) {
        self.units.push(unit);
    }

    pub fn add_import(&mut self, import: &str) {
        self.imports.push(import.to_string());
    }

    pub fn find_unit(&self, name: &str) -> Option<&UnitSkeleton> {
        self.units.iter().find(|u| u.name == name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitSkeleton {
    pub name: String,
    pub kind: UnitKind,
    pub signature: Option<String>,
    pub docstring: Option<String>,
    pub features: Vec<String>,
    pub dependencies: Vec<String>,
    pub body: Option<String>,
    pub visibility: Visibility,
}

impl UnitSkeleton {
    pub fn new(name: &str, kind: UnitKind) -> Self {
        Self {
            name: name.to_string(),
            kind,
            signature: None,
            docstring: None,
            features: Vec::new(),
            dependencies: Vec::new(),
            body: None,
            visibility: Visibility::Public,
        }
    }

    pub fn with_signature(mut self, sig: &str) -> Self {
        self.signature = Some(sig.to_string());
        self
    }

    pub fn with_docstring(mut self, doc: &str) -> Self {
        self.docstring = Some(doc.to_string());
        self
    }

    pub fn with_features(mut self, features: Vec<String>) -> Self {
        self.features = features;
        self
    }

    pub fn with_body(mut self, body: &str) -> Self {
        self.body = Some(body.to_string());
        self
    }

    pub fn with_visibility(mut self, visibility: Visibility) -> Self {
        self.visibility = visibility;
        self
    }

    pub fn add_dependency(&mut self, dep: &str) {
        self.dependencies.push(dep.to_string());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitKind {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Module,
    Constant,
    Variable,
}

impl UnitKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::Method => "method",
            Self::Class => "class",
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Interface => "interface",
            Self::Trait => "trait",
            Self::Module => "module",
            Self::Constant => "constant",
            Self::Variable => "variable",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    Public,
    Private,
    Protected,
    Internal,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Private => "private",
            Self::Protected => "protected",
            Self::Internal => "internal",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skeleton_file() {
        let mut skeleton = RepoSkeleton::new(PathBuf::from("src"), "python");

        let mut file = SkeletonFile::new(PathBuf::from("src/main.py"), "python");
        file.add_unit(UnitSkeleton::new("main", UnitKind::Function));
        file.add_import("sys");

        skeleton.add_file(file);

        assert_eq!(skeleton.files.len(), 1);
        assert_eq!(skeleton.files[0].units.len(), 1);
    }

    #[test]
    fn test_topological_order() {
        let mut skeleton = RepoSkeleton::new(PathBuf::from("src"), "python");

        // For topological sort to work, imports must resolve to actual file paths
        // Using ./b.py which resolves to src/b.py from src/a.py
        let mut file_a = SkeletonFile::new(PathBuf::from("src/a.py"), "python");
        file_a.add_import("./b.py");

        let file_b = SkeletonFile::new(PathBuf::from("src/b.py"), "python");

        skeleton.add_file(file_a);
        skeleton.add_file(file_b);

        let ordered = skeleton.topological_order();
        assert_eq!(ordered.len(), 2);
        // b.py should come before a.py since a.py imports b.py
        let b_pos = ordered
            .iter()
            .position(|f| f.path == PathBuf::from("src/b.py"))
            .unwrap();
        let a_pos = ordered
            .iter()
            .position(|f| f.path == PathBuf::from("src/a.py"))
            .unwrap();
        assert!(
            b_pos < a_pos,
            "b.py (pos {}) should come before a.py (pos {})",
            b_pos,
            a_pos
        );
    }

    #[test]
    fn test_skeleton_serialization() {
        let mut skeleton = RepoSkeleton::new(PathBuf::from("src"), "rust");
        let file = SkeletonFile::new(PathBuf::from("src/main.rs"), "rust");
        skeleton.add_file(file);

        let json = serde_json::to_string(&skeleton).unwrap();
        let deserialized: RepoSkeleton = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.files.len(), 1);
    }
}

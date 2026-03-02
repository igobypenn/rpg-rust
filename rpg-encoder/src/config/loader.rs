use crate::embedding::EmbeddingConfig;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct RpgConfig {
    #[serde(default)]
    pub embedding: Option<EmbeddingConfig>,
}

impl Default for RpgConfig {
    fn default() -> Self {
        Self {
            embedding: Some(EmbeddingConfig::default()),
        }
    }
}

pub struct ConfigLoader {
    global_path: PathBuf,
    project_path: PathBuf,
}

impl ConfigLoader {
    pub fn new() -> Self {
        let global_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rpg")
            .join("embedding.yaml");

        let project_path = PathBuf::from(".rpg").join("embedding.yaml");

        Self {
            global_path,
            project_path,
        }
    }

    pub fn with_paths(global: PathBuf, project: PathBuf) -> Self {
        Self {
            global_path: global,
            project_path: project,
        }
    }

    pub fn global_path(&self) -> &Path {
        &self.global_path
    }

    pub fn project_path(&self) -> &Path {
        &self.project_path
    }

    pub fn load(&self) -> RpgConfig {
        let mut config = RpgConfig::default();

        if self.global_path.exists() {
            if let Ok(contents) = fs::read_to_string(&self.global_path) {
                if let Ok(global) = serde_yaml::from_str::<RpgConfig>(&contents) {
                    config = self.merge_configs(config, global);
                }
            }
        }

        if self.project_path.exists() {
            if let Ok(contents) = fs::read_to_string(&self.project_path) {
                if let Ok(project) = serde_yaml::from_str::<RpgConfig>(&contents) {
                    config = self.merge_configs(config, project);
                }
            }
        }

        config
    }

    pub fn load_from<P: AsRef<Path>>(project_root: P) -> RpgConfig {
        let project_root = project_root.as_ref();
        let global_path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rpg")
            .join("embedding.yaml");

        let project_path = project_root.join(".rpg").join("embedding.yaml");

        let loader = Self::with_paths(global_path, project_path);
        loader.load()
    }

    fn merge_configs(&self, base: RpgConfig, override_config: RpgConfig) -> RpgConfig {
        let embedding = match (base.embedding, override_config.embedding) {
            (Some(base_emb), Some(override_emb)) => {
                Some(EmbeddingConfig::merge(base_emb, override_emb))
            }
            (Some(base_emb), None) => Some(base_emb),
            (None, override_emb) => override_emb,
        };

        RpgConfig { embedding }
    }
}

impl Default for ConfigLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_missing_config() {
        let temp = TempDir::new().unwrap();
        let loader = ConfigLoader::with_paths(
            temp.path().join("global.yaml"),
            temp.path().join("project.yaml"),
        );

        let config = loader.load();
        assert!(config.embedding.is_some());
    }

    #[test]
    fn test_load_project_config() {
        let temp = TempDir::new().unwrap();
        let project_path = temp.path().join("project.yaml");

        fs::write(
            &project_path,
            r#"
embedding:
  thresholds:
    similarity_threshold: 0.90
"#,
        )
        .unwrap();

        let loader = ConfigLoader::with_paths(temp.path().join("global.yaml"), project_path);

        let config = loader.load();
        assert_eq!(
            config.embedding.unwrap().thresholds.similarity_threshold,
            0.90
        );
    }

    #[test]
    fn test_config_precedence() {
        let temp = TempDir::new().unwrap();
        let global_path = temp.path().join("global.yaml");
        let project_path = temp.path().join("project.yaml");

        fs::write(
            &global_path,
            r#"
embedding:
  thresholds:
    similarity_threshold: 0.85
"#,
        )
        .unwrap();

        fs::write(
            &project_path,
            r#"
embedding:
  thresholds:
    similarity_threshold: 0.92
"#,
        )
        .unwrap();

        let loader = ConfigLoader::with_paths(global_path, project_path);
        let config = loader.load();

        assert_eq!(
            config.embedding.unwrap().thresholds.similarity_threshold,
            0.92
        );
    }
}

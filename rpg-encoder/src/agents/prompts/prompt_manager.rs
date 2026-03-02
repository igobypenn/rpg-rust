//! Simple template engine for LLM prompts.
//!
//! Provides `{{variable}}` replacement with optional filesystem override.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

/// Template source location.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum TemplateSource {
    Embedded(&'static str),
    Filesystem(PathBuf),
}

/// A prompt template with simple `{{variable}}` substitution.
#[derive(Debug, Clone)]
pub struct PromptTemplate {
    content: String,
    #[allow(dead_code)]
    source: TemplateSource,
}

impl PromptTemplate {
    /// Create a template from embedded content.
    pub fn embedded(content: &'static str) -> Self {
        Self {
            content: content.to_string(),
            source: TemplateSource::Embedded(content),
        }
    }

    /// Create a template from file content.
    pub fn from_file(path: &PathBuf) -> Option<Self> {
        let content = std::fs::read_to_string(path).ok()?;
        Some(Self {
            content,
            source: TemplateSource::Filesystem(path.clone()),
        })
    }

    /// Render the template with the given variables.
    ///
    /// Replaces `{{variable}}` patterns with values.
    pub fn render(&self, vars: &HashMap<String, String>) -> String {
        let mut result = self.content.clone();
        for (key, value) in vars {
            result = result.replace(&format!("{{{{{}}}}}", key), value);
        }
        result
    }
}

/// Manager for prompt templates with filesystem override support.
#[derive(Debug)]
pub struct PromptManager {
    prompts_dir: Option<PathBuf>,
    cache: RefCell<HashMap<String, PromptTemplate>>,
}

impl PromptManager {
    /// Create a new prompt manager.
    pub fn new(prompts_dir: Option<PathBuf>) -> Self {
        Self {
            prompts_dir,
            cache: RefCell::new(HashMap::new()),
        }
    }

    /// Get a template by name.
    ///
    /// First checks the filesystem override directory, then falls back to embedded.
    pub fn get(&self, name: &str, embedded: &'static str) -> PromptTemplate {
        let cached = self.cache.borrow().get(name).cloned();
        if let Some(template) = cached {
            return template;
        }

        let template = if let Some(ref dir) = self.prompts_dir {
            let path = dir.join(format!("{}.txt", name));
            PromptTemplate::from_file(&path).unwrap_or_else(|| PromptTemplate::embedded(embedded))
        } else {
            PromptTemplate::embedded(embedded)
        };
        self.cache
            .borrow_mut()
            .insert(name.to_string(), template.clone());
        template
    }

    /// Create a prompt manager with filesystem override directory.
    pub fn with_prompts_dir(mut self, dir: PathBuf) -> Self {
        self.prompts_dir = Some(dir);
        self.cache.borrow_mut().clear();
        self
    }

    /// Clear the template cache.
    pub fn clear(&self) {
        self.cache.borrow_mut().clear();
    }

    /// Check if a custom template exists on the filesystem.
    pub fn has_custom(&self, name: &str) -> bool {
        if let Some(ref dir) = self.prompts_dir {
            let path = dir.join(format!("{}.txt", name));
            std::fs::metadata(&path)
                .map(|m| m.is_file())
                .unwrap_or(false)
        } else {
            false
        }
    }
}

impl Clone for PromptManager {
    fn clone(&self) -> Self {
        Self {
            prompts_dir: self.prompts_dir.clone(),
            cache: RefCell::new(self.cache.borrow().clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_render() {
        let template = PromptTemplate::embedded("Hello {{name}}!");
        let mut vars = HashMap::new();
        vars.insert("name".to_string(), "World".to_string());
        assert_eq!(template.render(&vars), "Hello World!");
    }

    #[test]
    fn test_prompt_manager_default() {
        let mut manager = PromptManager::new(None);
        let template = manager.get("test", "Default: {{value}}");
        let mut vars = HashMap::new();
        vars.insert("value".to_string(), "test".to_string());
        assert_eq!(template.render(&vars), "Default: test");
    }

    #[test]
    fn test_prompt_manager_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("custom.txt");

        std::fs::write(&file_path, "Custom: {{value}}").unwrap();

        let mut manager = PromptManager::new(None).with_prompts_dir(dir.path().to_path_buf());
        let template = manager.get("custom", "Default: {{value}}");

        let mut vars = HashMap::new();
        vars.insert("value".to_string(), "test".to_string());
        assert_eq!(template.render(&vars), "Custom: test");
    }

    #[test]
    fn test_prompt_manager_override() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        std::fs::write(&file_path, "Override: {{value}}").unwrap();

        let mut manager = PromptManager::new(None).with_prompts_dir(dir.path().to_path_buf());
        let template = manager.get("test", "Default: {{value}}");

        let mut vars = HashMap::new();
        vars.insert("value".to_string(), "test".to_string());
        assert_eq!(template.render(&vars), "Override: test");
    }
}

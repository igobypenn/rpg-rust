use std::path::PathBuf;

use crate::error::Result;
use crate::parser::ParserRegistry;

pub struct FileWalker {
    root: PathBuf,
    ignore_file_name: String,
    max_depth: Option<usize>,
}

impl FileWalker {
    #[must_use = "FileWalker must be used"]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            ignore_file_name: ".rpgignore".to_string(),
            max_depth: None,
        }
    }

    pub fn with_ignore_file(mut self, name: impl Into<String>) -> Self {
        self.ignore_file_name = name.into();
        self
    }

    pub fn with_max_depth(mut self, depth: usize) -> Self {
        self.max_depth = Some(depth);
        self
    }

    pub fn walk(&self) -> Result<Vec<PathBuf>> {
        let mut files = Vec::new();

        let mut builder = ignore::WalkBuilder::new(&self.root);
        builder
            .hidden(false)
            .git_ignore(true)
            .git_global(false)
            .git_exclude(true)
            .ignore(true)
            .add_custom_ignore_filename(&self.ignore_file_name);

        if let Some(depth) = self.max_depth {
            builder.max_depth(Some(depth));
        }

        for result in builder.build() {
            match result {
                Ok(entry) => {
                    let path = entry.path().to_path_buf();
                    if path.is_file() {
                        files.push(path);
                    }
                }
                Err(err) => {
                    tracing::warn!("Walk error: {}", err);
                }
            }
        }

        Ok(files)
    }

    pub fn walk_with_parser_filter(&self, registry: &ParserRegistry) -> Result<Vec<PathBuf>> {
        let files = self.walk()?;
        Ok(files
            .into_iter()
            .filter(|p| registry.has_parser(p))
            .collect())
    }
}

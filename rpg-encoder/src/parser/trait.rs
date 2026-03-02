use std::path::Path;

use crate::core::NodeCategory;
use crate::error::Result;

use super::types::ParseResult;

pub trait LanguageParser: Send + Sync {
    fn language_name(&self) -> &str;

    fn file_extensions(&self) -> &[&str];

    fn default_category(&self) -> NodeCategory {
        NodeCategory::File
    }

    fn can_parse(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| self.file_extensions().contains(&ext))
    }

    fn parse(&self, source: &str, path: &Path) -> Result<ParseResult>;
}

pub struct ParserRegistry {
    parsers: Vec<Box<dyn LanguageParser>>,
    extension_map: std::collections::HashMap<String, usize>,
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: Vec::new(),
            extension_map: std::collections::HashMap::new(),
        }
    }

    pub fn register(&mut self, parser: Box<dyn LanguageParser>) {
        let idx = self.parsers.len();
        for ext in parser.file_extensions() {
            self.extension_map.insert(ext.to_string(), idx);
        }
        self.parsers.push(parser);
    }

    pub fn get_parser(&self, path: &Path) -> Option<&dyn LanguageParser> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(|ext| self.extension_map.get(ext))
            .and_then(|&idx| self.parsers.get(idx))
            .map(|p| p.as_ref())
    }

    pub fn has_parser(&self, path: &Path) -> bool {
        self.get_parser(path).is_some()
    }

    pub fn languages(&self) -> Vec<&str> {
        self.parsers.iter().map(|p| p.language_name()).collect()
    }
}

#[macro_export]
macro_rules! register_parser {
    ($registry:expr, $parser:ty) => {
        let parser = <$parser>::new().map_err(|e| {
            $crate::error::RpgError::parser_init(stringify!($parser), e.to_string())
        })?;
        $registry.register(Box::new(parser));
    };
}

#[macro_export]
macro_rules! register_parsers {
    ($registry:expr, $($parser:ty),* $(,)?) => {
        $(
            $crate::register_parser!($registry, $parser);
        )*
    };
}
